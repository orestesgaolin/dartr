// Dart source: pkg/analyzer/lib/src/dart/resolver/type_property_resolver.dart

//! `TypePropertyResolver`: looks up a property (getter, setter or method)
//! with a name in a receiver type: the interface members, extension
//! members, record fields, `call` of function types, with the recovery and
//! the nullable-receiver handling of the analyzer.
//!
//! `resolveStaticExtension` (static extensions, an experiment) is not
//! ported yet (unit C6).

use dartr_ast::{
    BinaryExpression, CascadeExpression, Expression, FunctionExpressionInvocation, Id,
    MethodInvocation, MethodReferenceExpression, NodeId, RelationalPattern, SuperExpression,
};
use dartr_diagnostics::{LocatableDiagnostic, diag};
use dartr_element::{EId, ElemRef, InterfaceElement, Nullability, TypeId, TypeKind};
use dartr_typesystem::TypeExt;
use dartr_typesystem::inheritance_manager3::{GetMemberOptions, InheritanceManager3, Name};
use dartr_typesystem::lookup;

use crate::extension_member_resolver;
use crate::resolution_result::{ResolutionResult, record_field_by_name};
use crate::resolver::ResolverVisitor;

/// The arguments of Dart `TypePropertyResolver.resolve(...)`.
#[derive(Clone, Copy, Debug)]
pub struct PropertyQuery<'s> {
    /// Dart `receiver`; `None` for an implicit `this` (or a cascade target).
    pub receiver: Option<Id<Expression>>,
    pub receiver_type: TypeId,
    pub name: &'s str,
    pub has_read: bool,
    pub has_write: bool,
    /// The node to report a nullable dereference on.
    pub property_error_entity: NodeId,
    /// The node to report an ambiguous extension on.
    pub name_error_entity: NodeId,
    /// Dart `parentNode`.
    pub parent_node: Option<NodeId>,
}

/// The state of one `resolve` call (the Dart fields `_needsGetterError`,
/// ...).
struct State<'s> {
    receiver: Option<Id<Expression>>,
    name: &'s str,
    has_read: bool,
    has_write: bool,
    name_error_entity: NodeId,
    needs_getter_error: bool,
    reported_getter_error: bool,
    getter_requested: Option<ElemRef>,
    getter_recovery: Option<ElemRef>,
    needs_setter_error: bool,
    reported_setter_error: bool,
    setter_requested: Option<ElemRef>,
    setter_recovery: Option<ElemRef>,
}

impl State<'_> {
    fn has_getter_or_setter(&self) -> bool {
        self.getter_requested.is_some() || self.setter_requested.is_some()
    }

    /// Dart `_toResult`.
    fn to_result(&self) -> ResolutionResult {
        ResolutionResult {
            getter: self.getter_requested.or(self.getter_recovery),
            // Parser recovery resulting in an empty property name should not
            // be reported as an undefined getter.
            needs_getter_error: self.needs_getter_error
                && !self.name.is_empty()
                && !self.reported_getter_error,
            is_getter_invalid: self.needs_getter_error || self.reported_getter_error,
            setter: self.setter_requested.or(self.setter_recovery),
            needs_setter_error: self.needs_setter_error && !self.reported_setter_error,
            call_function_type: None,
            record_field: None,
        }
    }
}

/// Dart `TypePropertyResolver.resolve`.
pub fn resolve(rv: &mut ResolverVisitor<'_>, q: PropertyQuery<'_>) -> ResolutionResult {
    let mut s = State {
        receiver: q.receiver,
        name: q.name,
        has_read: q.has_read,
        has_write: q.has_write,
        name_error_entity: q.name_error_entity,
        needs_getter_error: false,
        reported_getter_error: false,
        getter_requested: None,
        getter_recovery: None,
        needs_setter_error: false,
        reported_setter_error: false,
        setter_requested: None,
        setter_recovery: None,
    };
    let ts = rv.type_system;
    let ctx = rv.ctx;
    let object_type = ctx.tp.object_type();
    let mut receiver_type = q.receiver_type;

    if q.name == "new" {
        s.needs_getter_error = true;
        s.needs_setter_error = true;
        return s.to_result();
    }

    if ts.is_dynamic_bounded(receiver_type) || ts.is_invalid_bounded(receiver_type) {
        lookup_interface_type(rv, &mut s, object_type, false);
        s.needs_getter_error = false;
        s.needs_setter_error = false;
        return s.to_result();
    }

    let is_nullable = if ctx.is_extension_type(receiver_type) {
        ctx.nullability_suffix(receiver_type) == Nullability::Question
    } else {
        ts.is_potentially_nullable(receiver_type)
    };

    if is_nullable {
        lookup_interface_type(rv, &mut s, object_type, true);
        if s.has_getter_or_setter() {
            return s.to_result();
        }
        lookup_extension(rv, &mut s, receiver_type);
        if s.has_getter_or_setter() {
            return s.to_result();
        }

        let parent_node = q.parent_node.or_else(|| match q.receiver {
            Some(receiver) => rv.ast.parent(receiver),
            None => rv.ast.parent(q.property_error_entity),
        });
        let locatable = nullable_dereference_diagnostic(rv, parent_node, q.name);
        rv.report_nullable_dereference(locatable, q.property_error_entity, receiver_type);
        s.reported_getter_error = true;
        s.reported_setter_error = true;

        // Recovery, get some resolution.
        receiver_type = ts.resolve_to_bound(receiver_type);
        if matches!(ctx.ty(receiver_type), TypeKind::Interface { .. }) {
            lookup_interface_type(rv, &mut s, receiver_type, true);
        }
        return s.to_result();
    }

    let receiver_type_resolved = ts.resolve_to_bound(receiver_type);

    if matches!(ctx.ty(receiver_type_resolved), TypeKind::Interface { .. }) {
        lookup_interface_type(rv, &mut s, receiver_type_resolved, true);
        if s.has_getter_or_setter() {
            return s.to_result();
        }
        if ctx.is_dart_core_function(receiver_type_resolved) && s.name == "call" {
            s.needs_getter_error = false;
            s.needs_setter_error = false;
            return s.to_result();
        }
    }

    if matches!(ctx.ty(receiver_type_resolved), TypeKind::Function(_)) && s.name == "call" {
        return ResolutionResult {
            needs_getter_error: false,
            needs_setter_error: false,
            call_function_type: Some(receiver_type_resolved),
            ..ResolutionResult::default()
        };
    }

    if matches!(ctx.ty(receiver_type_resolved), TypeKind::Never(_)) {
        lookup_interface_type(rv, &mut s, object_type, true);
        s.needs_getter_error = false;
        s.needs_setter_error = false;
        return s.to_result();
    }

    if matches!(ctx.ty(receiver_type_resolved), TypeKind::Record { .. }) {
        if let Some(field) = record_field_by_name(&ctx, receiver_type_resolved, q.name) {
            return ResolutionResult {
                record_field: Some(field),
                needs_getter_error: false,
                ..ResolutionResult::default()
            };
        }
        s.needs_getter_error = true;
        s.needs_setter_error = true;
    }

    lookup_extension(rv, &mut s, receiver_type);
    if s.has_getter_or_setter() {
        return s.to_result();
    }

    lookup_interface_type(rv, &mut s, object_type, true);
    s.to_result()
}

/// The diagnostic of a nullable dereference (the `parentNode` checks of
/// Dart `resolve`).
fn nullable_dereference_diagnostic(
    rv: &ResolverVisitor<'_>,
    parent_node: Option<NodeId>,
    name: &str,
) -> LocatableDiagnostic {
    let ast = &*rv.ast;
    let Some(mut parent) = parent_node else {
        return diag::unchecked_invocation_of_nullable_value();
    };
    if let Some(cascade) = ast.cast::<CascadeExpression>(parent) {
        if let Some(&first) = ast.list(ast[cascade].cascade_sections).first() {
            parent = first.raw();
        }
    }
    if ast.is::<BinaryExpression>(parent) || ast.is::<RelationalPattern>(parent) {
        diag::unchecked_operator_invocation_of_nullable_value(name)
    } else if ast.is::<MethodInvocation>(parent) || ast.is::<MethodReferenceExpression>(parent) {
        diag::unchecked_method_invocation_of_nullable_value(name)
    } else if ast.is::<FunctionExpressionInvocation>(parent) {
        diag::unchecked_invocation_of_nullable_value()
    } else {
        diag::unchecked_property_access_of_nullable_value(name)
    }
}

/// Dart `_lookupExtension`.
fn lookup_extension(rv: &mut ResolverVisitor<'_>, s: &mut State<'_>, ty: TypeId) {
    let getter_name = Name::for_library(&rv.ctx, Some(rv.unit.library), s.name);
    let result = extension_member_resolver::find_extension(rv, ty, s.name_error_entity, &getter_name);
    s.reported_getter_error = result.is_ambiguous;
    s.reported_setter_error = result.is_ambiguous;
    if result.getter.is_some() {
        s.needs_getter_error = false;
        s.getter_requested = result.getter;
    }
    if result.setter.is_some() {
        s.needs_setter_error = false;
        s.setter_requested = result.setter;
    }
}

/// Dart `_lookupInterfaceType`.
fn lookup_interface_type(
    rv: &mut ResolverVisitor<'_>,
    s: &mut State<'_>,
    ty: TypeId,
    recover_with_static: bool,
) {
    let ctx = rv.ctx;
    let library = rv.unit.library;
    let is_super = s.receiver.is_some_and(|r| rv.ast.is::<SuperExpression>(r));
    let inheritance = InheritanceManager3::new(ctx);
    let options = GetMemberOptions {
        for_super: is_super,
        ..GetMemberOptions::default()
    };
    let element: Option<EId<InterfaceElement>> = ctx.interface_element(ty);
    let getter_name = Name::for_library(&ctx, Some(library), s.name);
    let setter_name = Name::for_library(&ctx, Some(library), &format!("{}=", s.name));

    if s.has_read {
        s.getter_requested = inheritance.get_member3(ty, getter_name, options);
        s.needs_getter_error = s.getter_requested.is_none();
        if s.getter_requested.is_none() && recover_with_static {
            if let Some(class) = element
                && s.getter_recovery.is_none()
            {
                s.getter_recovery = lookup::look_up_static_getter(&ctx, class, s.name, library)
                    .map(|g| ElemRef::Base(g.raw()))
                    .or_else(|| {
                        lookup::look_up_static_method(&ctx, class, s.name, library)
                            .map(|m| ElemRef::Base(m.raw()))
                    });
            }
            s.needs_getter_error = s.getter_recovery.is_none();
        }
    }

    if s.has_write {
        s.setter_requested = inheritance.get_member3(ty, setter_name, options);
        s.needs_setter_error = s.setter_requested.is_none();
        if s.setter_requested.is_none() && recover_with_static {
            if let Some(class) = element
                && s.setter_recovery.is_none()
            {
                s.setter_recovery = lookup::look_up_static_setter(&ctx, class, s.name, library)
                    .map(|g| ElemRef::Base(g.raw()));
            }
            s.needs_setter_error = s.setter_recovery.is_none();
        }
    }

    // If we wanted a getter, but it is not in the interface, then check if
    // there is the setter, i.e. the basename at all. If there is, we should
    // not check extensions.
    if s.has_read && s.getter_requested.is_none() {
        s.setter_requested = inheritance.get_member3(ty, setter_name, options);
    }
    if s.has_write && s.setter_requested.is_none() {
        s.getter_requested = inheritance.get_member3(ty, getter_name, options);
    }
}

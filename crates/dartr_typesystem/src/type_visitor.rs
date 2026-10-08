// Dart source: pkg/analyzer/lib/src/dart/element/type_visitor.dart and
// pkg/analyzer/lib/dart/element/type_visitor.dart (UnifyingTypeVisitor)

//! `RecursiveTypeVisitor`: visits a type tree until a visit method returns
//! `false`. Other Dart visitors (`TypeVisitor`, `InferenceTypeVisitor`) are
//! a `match` on [`TypeKind`] in Rust.

use dartr_element::{Ctx, TypeId, TypeKind};

use crate::type_ext::TypeExt;

/// `RecursiveTypeVisitor` (a `UnifyingTypeVisitor<bool>`): every `visitX`
/// that is not overridden calls [`RecursiveTypeVisitor::visit_dart_type`].
///
/// Dart throws for `UnknownInferredType` (it is not an
/// `InferenceTypeVisitor`); Rust sends it to `visit_dart_type`.
pub trait RecursiveTypeVisitor<'a> {
    fn ctx(&self) -> Ctx<'a>;

    /// `includeTypeAliasArguments`.
    fn include_type_alias_arguments(&self) -> bool;

    /// `type.accept(this)`.
    fn visit(&mut self, t: TypeId) -> bool {
        match *self.ctx().ty(t) {
            TypeKind::Interface { .. } => self.visit_interface_type(t),
            TypeKind::Function(_) => self.visit_function_type(t),
            TypeKind::Record { .. } => self.visit_record_type(t),
            TypeKind::TypeParameter { .. } => self.visit_type_parameter_type(t),
            TypeKind::Dynamic
            | TypeKind::Void
            | TypeKind::Invalid
            | TypeKind::Unknown
            | TypeKind::Never(_) => self.visit_dart_type(t),
        }
    }

    /// `visitChildren(types)`: stops at the first `false`.
    fn visit_children(&mut self, types: &[TypeId]) -> bool {
        for &t in types {
            if !self.visit(t) {
                return false;
            }
        }
        true
    }

    fn visit_dart_type(&mut self, t: TypeId) -> bool {
        super_visit_dart_type(self, t)
    }

    fn visit_function_type(&mut self, t: TypeId) -> bool {
        super_visit_function_type(self, t)
    }

    fn visit_interface_type(&mut self, t: TypeId) -> bool {
        super_visit_interface_type(self, t)
    }

    fn visit_record_type(&mut self, t: TypeId) -> bool {
        super_visit_record_type(self, t)
    }

    fn visit_type_parameter_type(&mut self, t: TypeId) -> bool {
        super_visit_type_parameter_type(self, t)
    }
}

/// `_maybeTypeAliasArguments(type)`.
fn maybe_type_alias_arguments<'a, V: RecursiveTypeVisitor<'a> + ?Sized>(
    v: &V,
    t: TypeId,
) -> &'a [TypeId] {
    let ctx = v.ctx();
    if v.include_type_alias_arguments()
        && let Some(alias) = ctx.type_alias(t)
    {
        return ctx.list(ctx.alias(alias).args);
    }
    &[]
}

/// The base `visitDartType`.
pub fn super_visit_dart_type<'a, V: RecursiveTypeVisitor<'a> + ?Sized>(v: &mut V, t: TypeId) -> bool {
    let args = maybe_type_alias_arguments(v, t);
    v.visit_children(args);
    true
}

/// The base `visitFunctionType`.
pub fn super_visit_function_type<'a, V: RecursiveTypeVisitor<'a> + ?Sized>(
    v: &mut V,
    t: TypeId,
) -> bool {
    let ctx = v.ctx();
    let TypeKind::Function(f) = *ctx.ty(t) else {
        unreachable!()
    };
    let args = maybe_type_alias_arguments(v, t);
    if !v.visit_children(args) {
        return false;
    }
    if !v.visit(f.ret) {
        return false;
    }
    for &type_parameter in ctx.list(f.type_params) {
        if let Some(bound) = ctx.type_parameter_bound(type_parameter)
            && !v.visit(bound)
        {
            return false;
        }
    }
    for p in ctx.list(f.params) {
        if !v.visit(p.ty) {
            return false;
        }
    }
    true
}

/// The base `visitInterfaceType`.
pub fn super_visit_interface_type<'a, V: RecursiveTypeVisitor<'a> + ?Sized>(
    v: &mut V,
    t: TypeId,
) -> bool {
    let args = maybe_type_alias_arguments(v, t);
    v.visit_children(args) && v.visit_children(v.ctx().type_arguments(t))
}

/// The base `visitRecordType`.
pub fn super_visit_record_type<'a, V: RecursiveTypeVisitor<'a> + ?Sized>(
    v: &mut V,
    t: TypeId,
) -> bool {
    let ctx = v.ctx();
    let TypeKind::Record {
        positional, named, ..
    } = *ctx.ty(t)
    else {
        unreachable!()
    };
    let args = maybe_type_alias_arguments(v, t);
    if !v.visit_children(args) || !v.visit_children(ctx.list(positional)) {
        return false;
    }
    let named: Vec<TypeId> = ctx.list(named).iter().map(|f| f.ty).collect();
    v.visit_children(&named)
}

/// The base `visitTypeParameterType` (the bound is not visited).
pub fn super_visit_type_parameter_type<'a, V: RecursiveTypeVisitor<'a> + ?Sized>(
    v: &mut V,
    t: TypeId,
) -> bool {
    let args = maybe_type_alias_arguments(v, t);
    v.visit_children(args);
    true
}

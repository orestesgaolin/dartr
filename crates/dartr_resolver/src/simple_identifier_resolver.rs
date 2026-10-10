// Dart source: pkg/analyzer/lib/src/dart/resolver/simple_identifier_resolver.dart,
// pkg/analyzer/lib/src/generated/resolver.dart (checkReadOfNotAssignedLocalVariable)

//! `SimpleIdentifierResolver`: the element and the static type of a
//! `SimpleIdentifier` expression (local variables with flow analysis
//! promotion, formal parameters, top-level and instance members, types,
//! prefixes).

use dartr_ast::{
    Annotation, ConstructorDeclaration, ConstructorFieldInitializer, ConstructorName,
    FieldFormalParameter, ForEachPartsWithIdentifier, Id, ImportDirective, Label, MethodInvocation,
    NamedType, PrefixedIdentifier, PropertyAccess, SimpleIdentifier, Statement, SwitchMember,
};
use dartr_diagnostics::diag;
use dartr_element::{
    AnyElement, ElemRef, ElementId, ExecutableElement, ExtensionElement, InterfaceElement,
    PromotableElement, PropertyAccessorElement, Tag, TypeId, TypeKind, VariableElement,
};
use dartr_syntax::TokenType;
use dartr_typesystem::member;

use crate::ast_ext;
use crate::element_ext;
use crate::property_element_resolver::{self, PropertyElementResolverResult};
use crate::resolver::ResolverVisitor;

/// Dart `ResolverVisitor.visitSimpleIdentifier(node, contextType:)` (the
/// part before `insertGenericFunctionInstantiation`) =
/// `SimpleIdentifierResolver.resolve(node, contextType:)`.
pub fn visit_simple_identifier(
    rv: &mut ResolverVisitor<'_>,
    node: Id<SimpleIdentifier>,
    context_type: TypeId,
) {
    if in_declaration_context(rv, node) {
        return;
    }
    rv.check_unreachable_node(node);
    let element = rv.element(node);
    rv.check_read_of_not_assigned_local_variable(node, element);
    // Dart `_reportDeprecatedExportUse(node)` (wave D).
    let mut already_resolved = false;
    let property_result = resolve1(rv, node, context_type, &mut already_resolved);
    if !already_resolved {
        resolve2(rv, node, property_result, context_type);
    }
}

/// Dart `SimpleIdentifierImpl.inDeclarationContext()`.
pub fn in_declaration_context(rv: &ResolverVisitor<'_>, node: Id<SimpleIdentifier>) -> bool {
    let Some(parent) = rv.ast.parent(node) else {
        return false;
    };
    if let Some(import) = rv.ast.cast::<ImportDirective>(parent) {
        return rv.ast[import].prefix == Some(node);
    }
    if rv.ast.is::<Label>(parent) {
        let Some(parent2) = rv.ast.parent(parent) else {
            return false;
        };
        return rv.ast.is::<Statement>(parent2) || rv.ast.is::<SwitchMember>(parent2);
    }
    false
}

/// Dart `_resolve1`.
fn resolve1(
    rv: &mut ResolverVisitor<'_>,
    node: Id<SimpleIdentifier>,
    context_type: TypeId,
    already_resolved: &mut bool,
) -> Option<PropertyElementResolverResult> {
    // Synthetic identifiers have been already reported during parsing.
    if rv.ast.tokens.get(rv.ast[node].token).is_synthetic() {
        return None;
    }
    // Ignore nodes that should have been resolved before getting here.
    if in_declaration_context(rv, node) {
        return None;
    }
    if let Some(e) = rv.base_element(node)
        && (element_ext::is_local_variable(e) || e.is::<dartr_element::FormalParameterElement>())
    {
        return None;
    }
    let parent = rv.ast.parent(node).expect("parent");
    if rv.ast.is::<FieldFormalParameter>(parent) {
        return None;
    }
    if let Some(p) = rv.ast.cast::<ConstructorFieldInitializer>(parent)
        && rv.ast[p].field_name == node
    {
        return None;
    }
    if let Some(p) = rv.ast.cast::<Annotation>(parent)
        && rv.ast[p].constructor_name == Some(node)
    {
        return None;
    }

    // Otherwise, the node should be resolved.
    let mut has_read = true;
    let mut has_write = false;
    if let Some(p) = rv.ast.cast::<ForEachPartsWithIdentifier>(parent)
        && rv.ast[p].identifier == node
    {
        has_read = false;
        has_write = true;
    }

    let result =
        property_element_resolver::resolve_simple_identifier(rv, node, has_read, has_write);

    if let Some(call_function_type) = result.function_type_call_type {
        let static_type = crate::invocation_inference_helper::infer_tear_off(
            rv,
            node.upcast(),
            node,
            call_function_type,
            context_type,
        );
        rv.record_static_type(node, static_type);
        *already_resolved = true;
        return None;
    }
    if let Some(record_field) = result.record_field {
        rv.record_static_type(node, record_field.ty);
        *already_resolved = true;
        return None;
    }

    let mut element = if has_read {
        result.read_element()
    } else {
        result.write_element()
    };

    let enclosing_class = rv.enclosing_class.map(|c| c.raw());
    let element_base = element.map(|e| member::base_element(&rv.ctx, e));
    if is_factory_constructor_return_type(rv, node) && element_base != enclosing_class {
        let d = diag::invalid_factory_name_not_a_class();
        let d = at_node(rv, d, node);
        rv.report(d);
    } else if is_constructor_return_type(rv, node) && element_base != enclosing_class {
        // This error is now reported by the parser.
        element = None;
    } else if element_base.is_some_and(|e| e.tag() == Tag::Prefix) && !is_valid_as_prefix(rv, node)
    {
        let e = element_base.unwrap();
        if let Some(name) = rv.ctx.element_data(e).and_then(|d| d.name) {
            let name = rv.ctx.name_str(name).to_string();
            let d = diag::prefix_identifier_not_followed_by_dot(&name);
            let d = at_node(rv, d, node);
            rv.report(d);
        }
    } else if element.is_none() {
        let name = rv.lexeme(rv.ast[node].token).to_string();
        if name == "await" && rv.enclosing_function.is_some() {
            let d = at_node(rv, diag::undefined_identifier_await(), node);
            rv.report(d);
        } else if !should_ignore_undefined_identifier(rv, &name) {
            let d = at_node(rv, diag::undefined_identifier(&name), node);
            rv.report(d);
        }
    }
    rv.set_element(node, element);
    Some(result)
}

/// Dart `_resolve2`.
fn resolve2(
    rv: &mut ResolverVisitor<'_>,
    node: Id<SimpleIdentifier>,
    property_result: Option<PropertyElementResolverResult>,
    context_type: TypeId,
) {
    let ctx = rv.ctx;
    let tp = ctx.tp;
    let Some(element) = rv.element(node) else {
        record(rv, node, TypeId::INVALID, context_type);
        return;
    };
    let base = member::base_element(&ctx, element);

    if base.is::<ExtensionElement>() {
        set_extension_identifier_type(rv, node);
        return;
    }

    #[allow(clippy::needless_late_init)]
    let static_type;
    if base.is::<InterfaceElement>() {
        if is_expression_identifier(rv, node) {
            rv.record_static_type(node, tp.type_type());
        }
        return;
    } else if base.tag() == Tag::TypeAlias {
        let aliased = match ctx.any(base) {
            AnyElement::TypeAlias(a) => a.aliased_type.get(),
            _ => None,
        };
        let aliased_is_interface =
            aliased.is_some_and(|t| matches!(ctx.ty(t), TypeKind::Interface { .. }));
        if is_expression_identifier(rv, node) || !aliased_is_interface {
            rv.record_static_type(node, tp.type_type());
        }
        return;
    } else if base.tag() == Tag::Method {
        static_type = member::type_(&ctx, element);
    } else if base.is::<PropertyAccessorElement>() {
        static_type = property_result
            .and_then(|r| r.get_type)
            .unwrap_or_else(|| type_of_property(rv, element));
    } else if base.is::<ExecutableElement>() {
        static_type = member::type_(&ctx, element);
    } else if base.tag() == Tag::TypeParameter {
        static_type = tp.type_type();
    } else if base.is::<VariableElement>() {
        let is_read = ast_ext::simple_identifier_in_getter_context(rv.ast, node);
        static_type = local_variable_type(rv, node, base, is_read);
    } else if base.tag() == Tag::Prefix {
        let parent = rv.ast.parent(node).expect("parent");
        if rv
            .ast
            .cast::<PrefixedIdentifier>(parent)
            .is_some_and(|p| rv.ast[p].prefix == node)
            || rv
                .ast
                .cast::<MethodInvocation>(parent)
                .is_some_and(|m| rv.ast[m].target == Some(node.upcast()))
        {
            return;
        }
        static_type = TypeId::INVALID;
    } else if base == ElementId::DYNAMIC || base == ElementId::NEVER {
        static_type = tp.type_type();
    } else {
        static_type = TypeId::INVALID;
    }
    record(rv, node, static_type, context_type);
}

/// The end of Dart `_resolve2`: the tear-off inference of old language
/// versions and `recordStaticType`.
fn record(
    rv: &mut ResolverVisitor<'_>,
    node: Id<SimpleIdentifier>,
    mut static_type: TypeId,
    context_type: TypeId,
) {
    if !rv.is_constructor_tearoffs_enabled() {
        // Only perform a generic function instantiation on a
        // `PrefixedIdentifier` in pre-constructor-tearoffs code. In
        // constructor-tearoffs-enabled code, generic function instantiation
        // is performed at assignability check sites.
        static_type = crate::invocation_inference_helper::infer_tear_off(
            rv,
            node.upcast(),
            node,
            static_type,
            context_type,
        );
    }
    rv.record_static_type(node, static_type);
}

/// Dart `_resolver.localVariableTypeProvider.getType(node, isRead:)`.
fn local_variable_type(
    rv: &mut ResolverVisitor<'_>,
    node: Id<SimpleIdentifier>,
    variable: ElementId,
    is_read: bool,
) -> TypeId {
    if variable.is::<PromotableElement>() && rv.flow_analysis.is_active() {
        let ctx = rv.ctx;
        return rv
            .flow_analysis
            .local_variable_type(&ctx, node.upcast(), variable, is_read);
    }
    element_ext::variable_type(&rv.ctx, variable)
}

/// Dart `_getTypeOfProperty`.
fn type_of_property(rv: &ResolverVisitor<'_>, accessor: ElemRef) -> TypeId {
    let ctx = rv.ctx;
    let function_type = member::type_(&ctx, accessor);
    let TypeKind::Function(f) = *ctx.ty(function_type) else {
        return TypeId::DYNAMIC;
    };
    if member::base_element(&ctx, accessor).tag() == Tag::Setter {
        // Dart `functionType.normalParameterTypes`: the required positional
        // parameters.
        let params = ctx.list(f.params);
        if let Some(first) = params.iter().find(|p| p.kind.is_required_positional()) {
            return first.ty;
        }
        if let Some(getter) = member::corresponding_getter(&ctx, accessor) {
            let getter_type = member::type_(&ctx, getter);
            if let TypeKind::Function(g) = *ctx.ty(getter_type) {
                return g.ret;
            }
        }
        return TypeId::DYNAMIC;
    }
    f.ret
}

/// Dart `_isExpressionIdentifier`: `true` if [node] is not a type literal.
fn is_expression_identifier(rv: &ResolverVisitor<'_>, node: Id<SimpleIdentifier>) -> bool {
    if in_declaration_context(rv, node) {
        return false;
    }
    let Some(parent) = rv.ast.parent(node) else {
        return true;
    };
    if let Some(c) = rv.ast.cast::<ConstructorDeclaration>(parent)
        && rv.ast[c].type_name == Some(node)
    {
        return false;
    }
    if rv.ast.is::<ConstructorName>(parent)
        || rv.ast.is::<MethodInvocation>(parent)
        || rv
            .ast
            .cast::<PrefixedIdentifier>(parent)
            .is_some_and(|p| rv.ast[p].prefix == node)
        || rv.ast.is::<PropertyAccess>(parent)
        || rv.ast.is::<NamedType>(parent)
    {
        return false;
    }
    true
}

/// Dart `_isValidAsPrefix`.
fn is_valid_as_prefix(rv: &ResolverVisitor<'_>, node: Id<SimpleIdentifier>) -> bool {
    let Some(parent) = rv.ast.parent(node) else {
        return false;
    };
    if let Some(import) = rv.ast.cast::<ImportDirective>(parent) {
        return rv.ast[import].prefix == Some(node);
    }
    if rv.ast.is::<PrefixedIdentifier>(parent) {
        return true;
    }
    if let Some(m) = rv.ast.cast::<MethodInvocation>(parent) {
        return rv.ast[m].target == Some(node.upcast())
            && rv.ast[m]
                .operator
                .is_some_and(|op| rv.ast.tokens.ty(op) == TokenType::PERIOD);
    }
    false
}

/// Dart `_isConstructorReturnType`.
fn is_constructor_return_type(rv: &ResolverVisitor<'_>, node: Id<SimpleIdentifier>) -> bool {
    rv.ast
        .parent(node)
        .and_then(|p| rv.ast.cast::<ConstructorDeclaration>(p))
        .is_some_and(|c| rv.ast[c].type_name == Some(node))
}

/// Dart `_isFactoryConstructorReturnType`.
fn is_factory_constructor_return_type(
    rv: &ResolverVisitor<'_>,
    node: Id<SimpleIdentifier>,
) -> bool {
    rv.ast
        .parent(node)
        .and_then(|p| rv.ast.cast::<ConstructorDeclaration>(p))
        .is_some_and(|c| rv.ast[c].type_name == Some(node) && rv.ast[c].factory_keyword.is_some())
}

/// Dart `_setExtensionIdentifierType` (for a simple identifier).
fn set_extension_identifier_type(rv: &mut ResolverVisitor<'_>, node: Id<SimpleIdentifier>) {
    if in_declaration_context(rv, node) {
        return;
    }
    let mut current: dartr_ast::NodeId = node.raw();
    let mut parent = rv.ast.parent(current);
    if let Some(p) = parent.and_then(|p| rv.ast.cast::<PrefixedIdentifier>(p))
        && rv.ast[p].identifier == node
    {
        current = p.raw();
        parent = rv.ast.parent(p);
    }
    if let Some(parent) = parent {
        let ast = &*rv.ast;
        if ast.is::<dartr_ast::CommentReference>(parent)
            || ast
                .cast::<MethodInvocation>(parent)
                .is_some_and(|m| ast[m].target.map(|t| t.raw()) == Some(current))
            || ast
                .cast::<PrefixedIdentifier>(parent)
                .is_some_and(|p| ast[p].prefix.raw() == current)
            || ast
                .cast::<PropertyAccess>(parent)
                .is_some_and(|p| ast[p].target.map(|t| t.raw()) == Some(current))
        {
            return;
        }
    }
    let name = rv.lexeme(rv.ast[node].token).to_string();
    let offset = rv.ast.offset(current) as usize;
    let length = rv.ast.length(current) as usize;
    rv.report(diag::extension_as_expression(&name).at_offset(offset, length));
    if let Some(p) = rv.ast.cast::<PrefixedIdentifier>(current) {
        let identifier = rv.ast[p].identifier;
        rv.set_static_type(identifier, TypeId::DYNAMIC);
        rv.set_static_type(p, TypeId::DYNAMIC);
    } else {
        rv.set_static_type(node, TypeId::DYNAMIC);
    }
}

/// Dart `libraryFragment.shouldIgnoreUndefinedIdentifier(node)` for a
/// simple identifier: the name is shown by an import of a library that does
/// not exist.
fn should_ignore_undefined_identifier(rv: &ResolverVisitor<'_>, name: &str) -> bool {
    crate::method_invocation_resolver::should_ignore_undefined(rv, None, name)
}

/// `diagnostic.at(node)`.
fn at_node(
    rv: &ResolverVisitor<'_>,
    d: dartr_diagnostics::LocatableDiagnostic,
    node: Id<SimpleIdentifier>,
) -> dartr_diagnostics::LocatedDiagnostic {
    d.at_offset(rv.ast.offset(node) as usize, rv.ast.length(node) as usize)
}

impl<'a> ResolverVisitor<'a> {
    /// Dart `checkReadOfNotAssignedLocalVariable(node, element)`.
    pub fn check_read_of_not_assigned_local_variable(
        &mut self,
        node: Id<SimpleIdentifier>,
        element: Option<ElemRef>,
    ) {
        if !self.flow_analysis.is_active() {
            return;
        }
        if !ast_ext::simple_identifier_in_getter_context(self.ast, node) {
            return;
        }
        let Some(ElemRef::Base(element)) = element else {
            return;
        };
        let Some(promotable) = element.cast::<PromotableElement>() else {
            return;
        };
        let assigned = self.flow_analysis.is_definitely_assigned(promotable);
        let unassigned = self.flow_analysis.is_definitely_unassigned(promotable);
        let name = self.lexeme(self.ast[node].token).to_string();
        let offset = self.ast.offset(node) as usize;
        let length = self.ast.length(node) as usize;
        if element_ext::is_late(&self.ctx, element) {
            if unassigned {
                self.report(
                    diag::definitely_unassigned_late_local_variable(&name)
                        .at_offset(offset, length),
                );
            }
            return;
        }
        if !assigned {
            if element_ext::is_final(&self.ctx, element) {
                self.report(
                    diag::read_potentially_unassigned_final(&name).at_offset(offset, length),
                );
                return;
            }
            let ty = element_ext::variable_type(&self.ctx, element);
            if self.type_system.is_potentially_non_nullable(ty) {
                self.report(
                    diag::not_assigned_potentially_non_nullable_local_variable(&name)
                        .at_offset(offset, length),
                );
            }
        }
    }
}

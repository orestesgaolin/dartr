// Dart source: pkg/analyzer/lib/src/dart/resolver/property_element_resolver.dart,
// pkg/analyzer/lib/src/generated/resolver.dart (visitPropertyAccess,
// visitIndexExpression, _resolvePropertyAccessRhs,
// _resolvePropertyAccessRhs_common, _startNullAwareAccess),
// pkg/analyzer/lib/src/generated/super_context.dart (SuperContext.of)

//! `PropertyElementResolver`: the elements and the read types of property
//! accesses (`a.b`, `a?.b`, `..b`, `super.b`, `E(a).b`, `C.b`, `p.b` with an
//! import prefix), prefixed identifiers, index expressions (`a[i]`,
//! `a?[i]`, `..[i]`) and implicit `this` / cascade identifiers.
//!
//! The entry points of the core are [`visit_property_access`] and
//! [`visit_index_expression`] (Dart `ResolverVisitor.visitPropertyAccess`,
//! `visitIndexExpression`). The resolution of a left-hand side
//! (`resolveForWrite`) is in [`crate::assignment_expression_resolver`].
//!
//! Not ported yet: `resolveDotShorthand` (unit C8), static extensions
//! (`resolveStaticExtension`, an experiment), `reportDeprecatedExportUse`
//! and the null-safety dead code verifier (wave D).

use dartr_ast::{
    Annotation, AnonymousMethodBody, AnonymousMethodInvocation, CascadeExpression,
    ClassDeclaration, CompilationUnit, ConstructorDeclaration, ConstructorInitializer,
    EnumDeclaration, Expression, ExtensionDeclaration, ExtensionOverride, ExtensionTypeDeclaration,
    FieldDeclaration, Id, IndexExpression, MethodDeclaration, MixinDeclaration, NamedArgument,
    NodeId, PrefixedIdentifier, PropertyAccess, SimpleIdentifier, SuperExpression, TypeLiteral,
};
use dartr_diagnostics::{LocatableDiagnostic, diag};
use dartr_element::diagnostics::type_arg;
use dartr_element::{
    EId, ElemRef, ElementId, ExtensionElement, InstanceElement, InterfaceElement, PrefixElement,
    Tag, TypeId, TypeKind,
};
use dartr_flow::flow_analysis::{FlowAnalysis, PropertyTarget};
use dartr_flow::shared_type::{SharedTypeSchemaView, SharedTypeView};
use dartr_flow::type_analyzer::TypeAnalyzer;
use dartr_syntax::TokenType;
use dartr_typesystem::TypeExt;
use dartr_typesystem::inheritance_manager3::{GetMemberOptions, InheritanceManager3, Name};
use dartr_typesystem::lookup::{self, LookUpOptions};
use dartr_typesystem::member;
use dartr_typesystem::type_algebra::MapSubstitution;

use crate::extension_member_resolver::ExtensionResolutionResult;
use crate::resolver::ResolverVisitor;
use crate::type_property_resolver::{self, PropertyQuery};

// ------------------------------------------------------------------ visitors

/// Dart `ResolverVisitor.visitPropertyAccess(node, contextType: contextType)`.
pub fn visit_property_access(
    rv: &mut ResolverVisitor<'_>,
    node: Id<PropertyAccess>,
    context_type: TypeId,
) {
    // If [isDotShorthand] is set, cache the context type for resolution.
    let is_dot_shorthand = rv.is_dot_shorthand(node.upcast());
    if is_dot_shorthand {
        rv.push_dot_shorthand_context(node.raw(), SharedTypeSchemaView::new(context_type));
    }

    rv.check_unreachable_node(node);

    if let Some(target) = rv.ast[node].target {
        rv.analyze_expression(
            target,
            SharedTypeSchemaView::new(TypeId::UNKNOWN),
            true,
            false,
            false,
        );
        rv.pop_rewrite();
    }

    let property_name = rv.ast[node].property_name;
    rv.check_unreachable_node(property_name);
    resolve_property_access_rhs(rv, node, context_type, None);

    if is_dot_shorthand {
        rv.pop_dot_shorthand_context();
    }
}

/// Dart `ResolverVisitor.visitIndexExpression(node, contextType: contextType)`.
pub fn visit_index_expression(
    rv: &mut ResolverVisitor<'_>,
    node: Id<IndexExpression>,
    context_type: TypeId,
) {
    // If [isDotShorthand] is set, cache the context type for resolution.
    let is_dot_shorthand = rv.is_dot_shorthand(node.upcast());
    if is_dot_shorthand {
        rv.push_dot_shorthand_context(node.raw(), SharedTypeSchemaView::new(context_type));
    }

    rv.check_unreachable_node(node);

    if let Some(target) = rv.ast[node].target {
        rv.analyze_expression(
            target,
            SharedTypeSchemaView::new(TypeId::UNKNOWN),
            true,
            false,
            false,
        );
        rv.pop_rewrite();
    }
    let target_type = index_expression_real_target(rv, node).and_then(|t| rv.static_type(t));

    if index_expression_is_null_aware(rv, node) {
        let target = rv.ast[node].target;
        start_null_aware_access(rv, target);
        // Dart `nullSafetyDeadCodeVerifier.visitNode(node.index)` (wave D).
    }

    let result = resolve_index_expression(rv, node, true, false);

    let element = result.read_element();
    rv.set_element(node, element);

    let index = rv.ast[node].index;
    rv.resolve_expression(index, result.index_context_type.unwrap_or(TypeId::UNKNOWN));
    // Dart `checkIndexExpressionIndex(node.index, readElement:,
    // writeElement: null, whyNotPromoted:)` (wave D).

    let ctx = rv.ctx;
    let ty = if target_type == Some(TypeId::NEVER) {
        TypeId::NEVER
    } else if let Some(e) = element.filter(|&e| member::base_element(&ctx, e).tag() == Tag::Method)
    {
        member::return_type(&ctx, e)
    } else if target_type.is_some_and(|t| matches!(ctx.ty(t), TypeKind::Dynamic)) {
        TypeId::DYNAMIC
    } else {
        TypeId::INVALID
    };
    rv.record_static_type(node, ty);
    let replacement = rv.insert_generic_function_instantiation(node.upcast(), context_type);
    rv.insert_implicit_call_reference(replacement, context_type);
    // Dart `nullSafetyDeadCodeVerifier.verifyIndexExpression(node)` (wave D).

    if is_dot_shorthand {
        rv.pop_dot_shorthand_context();
    }
}

/// Dart `ResolverVisitor._resolvePropertyAccessRhs(node, contextType,
/// originalNode:)`.
pub fn resolve_property_access_rhs(
    rv: &mut ResolverVisitor<'_>,
    node: Id<PropertyAccess>,
    context_type: TypeId,
    original_node: Option<Id<PrefixedIdentifier>>,
) {
    if property_access_is_null_aware(rv, node) {
        let target = rv.ast[node].target;
        start_null_aware_access(rv, target);
        // Dart `nullSafetyDeadCodeVerifier.visitNode(node.propertyName)`
        // (wave D).
    }

    let result = resolve_property_access(rv, node, true, false, original_node);

    let property_name = rv.ast[node].property_name;
    resolve_property_access_rhs_common(rv, result, node.upcast(), property_name, context_type);
    // Dart `nullSafetyDeadCodeVerifier.verifyPropertyAccess(node)` (wave D).
}

/// Dart `ResolverVisitor._resolvePropertyAccessRhs_common(resolverResult,
/// node, propertyName, contextType)`.
pub fn resolve_property_access_rhs_common(
    rv: &mut ResolverVisitor<'_>,
    resolver_result: PropertyElementResolverResult,
    node: Id<Expression>,
    property_name: Id<SimpleIdentifier>,
    context_type: TypeId,
) {
    let ctx = rv.ctx;
    let element = resolver_result.read_element();

    rv.set_element(property_name, element);

    let mut ty = match element.map(|e| (e, member::base_element(&ctx, e).tag())) {
        Some((e, Tag::Method | Tag::Constructor)) => member::type_(&ctx, e),
        Some((_, Tag::Getter)) => resolver_result.get_type.unwrap_or(TypeId::INVALID),
        _ => {
            if let Some(t) = resolver_result.function_type_call_type {
                t
            } else if let Some(field) = resolver_result.record_field {
                field.ty
            } else if resolver_result.at_dynamic_target {
                TypeId::DYNAMIC
            } else {
                TypeId::INVALID
            }
        }
    };

    if !rv.is_constructor_tearoffs_enabled() {
        // Only perform a generic function instantiation on a
        // [PrefixedIdentifier] in pre-constructor-tearoffs code. In
        // constructor-tearoffs-enabled code, generic function instantiation
        // is performed at assignability check sites.
        ty = crate::invocation_inference_helper::infer_tear_off(
            rv,
            node,
            property_name,
            ty,
            context_type,
        );
    }

    rv.set_static_type(property_name, ty);
    rv.record_static_type(node, ty);
    let replacement = rv.insert_generic_function_instantiation(node, context_type);
    rv.insert_implicit_call_reference(replacement, context_type);
}

/// Dart `ResolverVisitor._startNullAwareAccess(target)`.
pub fn start_null_aware_access(rv: &mut ResolverVisitor<'_>, target: Option<Id<Expression>>) {
    if rv.flow_analysis.flow.is_none() {
        return;
    }
    let Some(target) = target else {
        // This means the property access target is the target of a
        // cascade. For this case, `node.isNullAware=true` means that the
        // cascade is null aware, but that has already been taken care of in
        // `visitCascadeExpression`. So there is nothing further to do.
        return;
    };
    if let Some(identifier) = rv.ast.cast::<SimpleIdentifier>(target)
        && rv
            .base_element(identifier)
            .is_some_and(|e| e.is::<InterfaceElement>())
    {
        // `?.` to access static methods is equivalent to `.`, so do nothing.
        return;
    }
    let mut expression = target;
    if let Some(e) = rv.ast.cast::<ExtensionOverride>(target) {
        let argument_list = rv.ast[e].argument_list;
        let arguments = rv.ast.list(rv.ast[argument_list].arguments).to_vec();
        if let [argument] = arguments[..] {
            expression = match rv.ast.cast::<NamedArgument>(argument) {
                Some(named) => rv.ast[named].argument_expression,
                None => Id::from_raw(argument.raw()),
            };
        }
    }
    let info = rv.flow_analysis.get_expression_info(Some(expression));
    let ty = rv.static_type(expression).unwrap_or(TypeId::DYNAMIC);
    let result = dartr_flow::null_shorting::TypeAnalysisNullShortingInterface::start_null_shorting(
        rv,
        (),
        info,
        SharedTypeView::new(ty),
        None,
    );
    rv.flow_analysis.store_expression_info(expression, result);
}

// ------------------------------------------------------------------ result

/// Dart `PropertyElementResolverResult`.
#[derive(Clone, Copy, Debug, Default)]
pub struct PropertyElementResolverResult {
    pub read_element_requested: Option<ElemRef>,
    pub read_element_recovery: Option<ElemRef>,
    pub write_element_requested: Option<ElemRef>,
    pub write_element_recovery: Option<ElemRef>,
    pub at_dynamic_target: bool,
    pub function_type_call_type: Option<TypeId>,
    pub record_field: Option<crate::resolution_result::RecordField>,
    pub get_type: Option<TypeId>,
    /// If an `IndexExpression` is resolved, the context type of the index
    /// (`_` if `[]` or `[]=` are not resolved or invalid). `None` = `_`.
    pub index_context_type: Option<TypeId>,
}

impl PropertyElementResolverResult {
    /// Dart `readElement2`.
    pub fn read_element(&self) -> Option<ElemRef> {
        self.read_element_requested.or(self.read_element_recovery)
    }

    /// Dart `writeElement2`.
    pub fn write_element(&self) -> Option<ElemRef> {
        self.write_element_requested.or(self.write_element_recovery)
    }
}

// ------------------------------------------------------------------ syntax

/// Dart `SimpleIdentifierImpl.ancestorCascade`.
pub fn ancestor_cascade(
    rv: &ResolverVisitor<'_>,
    node: Id<SimpleIdentifier>,
) -> Option<Id<CascadeExpression>> {
    let token = rv.ast[node].token;
    let previous = rv.ast.tokens.previous(token);
    let ty = rv.ast.tokens.ty(previous);
    if ty == TokenType::PERIOD_PERIOD || ty == TokenType::QUESTION_PERIOD_PERIOD {
        return rv.ast.this_or_ancestor_of_type::<CascadeExpression>(node);
    }
    None
}

/// Dart `CascadeExpressionImpl.isNullAware`.
pub fn cascade_is_null_aware(rv: &ResolverVisitor<'_>, node: Id<CascadeExpression>) -> bool {
    let target = rv.ast[node].target;
    let end = rv.ast.end_token(target.raw());
    let next = rv.ast.tokens.next(end);
    rv.ast.tokens.ty(next) == TokenType::QUESTION_PERIOD_PERIOD
}

/// The nearest enclosing cascade of [node] (Dart `_ancestorCascade`).
fn enclosing_cascade(rv: &ResolverVisitor<'_>, node: NodeId) -> Option<Id<CascadeExpression>> {
    let parent = rv.ast.parent(node)?;
    rv.ast.this_or_ancestor_of_type::<CascadeExpression>(parent)
}

/// Dart `PropertyAccessImpl.realTarget` (`None` only for invalid trees).
pub fn property_access_real_target(
    rv: &ResolverVisitor<'_>,
    node: Id<PropertyAccess>,
) -> Option<Id<Expression>> {
    if crate::ast_ext::property_access_is_cascaded(rv.ast, node) {
        return enclosing_cascade(rv, node.raw()).map(|c| rv.ast[c].target);
    }
    rv.ast[node].target
}

/// Dart `PropertyAccessImpl.isNullAware`.
pub fn property_access_is_null_aware(rv: &ResolverVisitor<'_>, node: Id<PropertyAccess>) -> bool {
    if crate::ast_ext::property_access_is_cascaded(rv.ast, node) {
        return enclosing_cascade(rv, node.raw()).is_some_and(|c| cascade_is_null_aware(rv, c));
    }
    let ty = rv.ast.tokens.ty(rv.ast[node].operator);
    ty == TokenType::QUESTION_PERIOD || ty == TokenType::QUESTION_PERIOD_PERIOD
}

/// Dart `IndexExpressionImpl.isCascaded`.
fn index_expression_is_cascaded(rv: &ResolverVisitor<'_>, node: Id<IndexExpression>) -> bool {
    rv.ast[node].period.is_some()
}

/// Dart `IndexExpressionImpl.realTarget` (`None` only for invalid trees).
pub fn index_expression_real_target(
    rv: &ResolverVisitor<'_>,
    node: Id<IndexExpression>,
) -> Option<Id<Expression>> {
    if index_expression_is_cascaded(rv, node) {
        return enclosing_cascade(rv, node.raw()).map(|c| rv.ast[c].target);
    }
    rv.ast[node].target
}

/// Dart `IndexExpressionImpl.isNullAware`.
pub fn index_expression_is_null_aware(rv: &ResolverVisitor<'_>, node: Id<IndexExpression>) -> bool {
    if index_expression_is_cascaded(rv, node) {
        return enclosing_cascade(rv, node.raw()).is_some_and(|c| cascade_is_null_aware(rv, c));
    }
    rv.ast[node].question.is_some()
}

/// Dart `SuperContext.of(expression) == SuperContext.valid`.
pub fn super_context_is_valid(rv: &ResolverVisitor<'_>, expression: Id<SuperExpression>) -> bool {
    let ast = &*rv.ast;
    let mut current = Some(expression.raw());
    while let Some(node) = current {
        if ast.is::<Annotation>(node) {
            return false;
        } else if ast.is::<AnonymousMethodBody>(node) {
            if let Some(invocation) = ast
                .parent(node)
                .and_then(|p| ast.cast::<AnonymousMethodInvocation>(p))
                && ast[invocation].parameters.is_none()
            {
                return false;
            }
        } else if ast.is::<ClassDeclaration>(node) {
            return true;
        } else if ast.is::<CompilationUnit>(node) {
            return false;
        } else if let Some(c) = ast.cast::<ConstructorDeclaration>(node) {
            if ast[c].factory_keyword.is_some() {
                return false;
            }
        } else if ast.is::<ConstructorInitializer>(node) {
            return false;
        } else if ast.is::<EnumDeclaration>(node) {
            return true;
        } else if ast.is::<ExtensionDeclaration>(node) || ast.is::<ExtensionTypeDeclaration>(node) {
            return false;
        } else if let Some(f) = ast.cast::<FieldDeclaration>(node) {
            if ast[f].static_keyword.is_some() {
                return false;
            }
            if ast[ast[f].fields].late_keyword.is_none() {
                return false;
            }
        } else if let Some(m) = ast.cast::<MethodDeclaration>(node) {
            if ast[m]
                .modifier_keyword
                .is_some_and(|k| ast.tokens.lexeme(k) == "static")
            {
                return false;
            }
        } else if ast.is::<MixinDeclaration>(node) {
            return true;
        }
        current = ast.parent(node);
    }
    false
}

/// The name of [node] (Dart `SimpleIdentifier.name`).
fn identifier_name(rv: &ResolverVisitor<'_>, node: Id<SimpleIdentifier>) -> String {
    rv.lexeme(rv.ast[node].token).to_string()
}

/// The name of [element] (Dart `element.name!`), empty if it has none.
fn element_name(rv: &ResolverVisitor<'_>, element: ElementId) -> String {
    rv.ctx.element_name(element).unwrap_or("").to_string()
}

/// Dart `ElementKind.displayName` of [element].
pub fn element_kind_display_name(element: ElementId) -> &'static str {
    match element.tag() {
        Tag::Class => "class",
        Tag::Enum => "enum",
        Tag::Mixin => "mixin",
        Tag::Extension => "extension",
        Tag::ExtensionType => "extension type",
        Tag::Field => "field",
        Tag::Getter => "getter",
        Tag::Setter => "setter",
        Tag::Method => "method",
        Tag::Constructor => "constructor",
        Tag::TopLevelFunction | Tag::LocalFunction => "function",
        Tag::TopLevelVariable => "top level variable",
        Tag::TypeAlias => "type alias",
        Tag::TypeParameter => "type parameter",
        Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter => "parameter",
        Tag::Prefix => "import prefix",
        Tag::LocalVariable
        | Tag::PatternVariable
        | Tag::BindPatternVariable
        | Tag::JoinPatternVariable => "local variable",
        Tag::Label => "label",
        Tag::Library => "library",
        Tag::GenericFunctionType => "generic function type",
        Tag::MultiplyDefined => "multiply defined element",
        Tag::Dynamic => "dynamic",
        Tag::Never => "Never",
    }
}

/// Dart `firstParameterType` of an executable element.
pub fn first_parameter_type(rv: &ResolverVisitor<'_>, element: ElemRef) -> Option<TypeId> {
    let ctx = rv.ctx;
    member::formal_parameters(&ctx, element)
        .first()
        .map(|&p| member::type_(&ctx, p))
}

/// Dart `ExecutableElement.isStatic`.
fn is_static(rv: &ResolverVisitor<'_>, element: ElemRef) -> bool {
    member::is_static(&rv.ctx, element)
}

// ------------------------------------------------------------------ resolve

/// Dart `PropertyElementResolver.resolveIndexExpression(node:, hasRead:,
/// hasWrite:)`.
pub fn resolve_index_expression(
    rv: &mut ResolverVisitor<'_>,
    node: Id<IndexExpression>,
    has_read: bool,
    has_write: bool,
) -> PropertyElementResolverResult {
    let Some(target) = index_expression_real_target(rv, node) else {
        return PropertyElementResolverResult::default();
    };

    if let Some(override_) = rv.ast.cast::<ExtensionOverride>(target) {
        let result = get_override_member(rv, override_, "[]");
        let extension_name = rv
            .base_element(override_)
            .map(|e| element_name(rv, e))
            .unwrap_or_default();

        // TODO(scheglov): Change ExtensionResolver to set `needsGetterError`.
        if has_read && result.getter.is_none() && !result.is_ambiguous {
            report_unresolved_index(
                rv,
                node,
                diag::undefined_extension_operator("[]", &extension_name),
            );
        }
        if has_write && result.setter.is_none() && !result.is_ambiguous {
            report_unresolved_index(
                rv,
                node,
                diag::undefined_extension_operator("[]=", &extension_name),
            );
        }
        return to_index_result(rv, result.getter, result.setter, false, has_read);
    }

    let ts = rv.type_system;
    let mut target_type = ts.resolve_to_bound(rv.type_or_throw(target));

    if matches!(rv.ctx.ty(target_type), TypeKind::Void) {
        // TODO(scheglov): Report directly in TypePropertyResolver?
        report_unresolved_index(rv, node, diag::use_of_void_result());
        return PropertyElementResolverResult::default();
    }

    if target_type == TypeId::NEVER {
        // TODO(scheglov): Report directly in TypePropertyResolver?
        let d = rv.at(diag::receiver_of_type_never(), target);
        rv.report(d);
        return PropertyElementResolverResult::default();
    }

    if index_expression_is_null_aware(rv, node) {
        target_type = ts.promote_to_non_null(target_type);
    }

    let left_bracket = rv.ast[node].left_bracket;
    let result = type_property_resolver::resolve_at_token(
        rv,
        PropertyQuery {
            receiver: Some(target),
            receiver_type: target_type,
            name: "[]",
            has_read,
            has_write,
            property_error_entity: node.raw(),
            name_error_entity: target.raw(),
            parent_node: None,
        },
        left_bracket,
    );

    let is_super = rv.ast.is::<SuperExpression>(target);
    if has_read && result.needs_getter_error {
        let t = type_arg(&rv.ctx, target_type);
        let d = if is_super {
            diag::undefined_super_operator("[]", t)
        } else {
            diag::undefined_operator("[]", t)
        };
        report_unresolved_index(rv, node, d);
    }
    if has_write && result.needs_setter_error {
        let t = type_arg(&rv.ctx, target_type);
        let d = if is_super {
            diag::undefined_super_operator("[]=", t)
        } else {
            diag::undefined_operator("[]=", t)
        };
        report_unresolved_index(rv, node, d);
    }

    let at_dynamic_target = matches!(rv.ctx.ty(target_type), TypeKind::Dynamic);
    to_index_result(
        rv,
        result.getter,
        result.setter,
        at_dynamic_target,
        has_read,
    )
}

/// Dart `PropertyElementResolver.resolvePrefixedIdentifier(node:, hasRead:,
/// hasWrite:, forAnnotation:)`.
pub fn resolve_prefixed_identifier(
    rv: &mut ResolverVisitor<'_>,
    node: Id<PrefixedIdentifier>,
    has_read: bool,
    has_write: bool,
    for_annotation: bool,
) -> PropertyElementResolverResult {
    let prefix = rv.ast[node].prefix;
    let identifier = rv.ast[node].identifier;

    if let Some(prefix_element) = rv
        .base_element(prefix)
        .and_then(|e| e.cast::<PrefixElement>())
    {
        return resolve_target_prefix_element(
            rv,
            prefix_element,
            identifier,
            has_read,
            has_write,
            for_annotation,
        );
    }

    resolve(
        rv,
        node.upcast(),
        prefix.upcast(),
        false,
        false,
        identifier,
        has_read,
        has_write,
        None,
    )
}

/// Dart `PropertyElementResolver.resolvePropertyAccess(node:, hasRead:,
/// hasWrite:, originalNode:)`.
pub fn resolve_property_access(
    rv: &mut ResolverVisitor<'_>,
    node: Id<PropertyAccess>,
    has_read: bool,
    has_write: bool,
    original_node: Option<Id<PrefixedIdentifier>>,
) -> PropertyElementResolverResult {
    let Some(target) = property_access_real_target(rv, node) else {
        return PropertyElementResolverResult::default();
    };
    let property_name = rv.ast[node].property_name;

    if let Some(override_) = rv.ast.cast::<ExtensionOverride>(target) {
        return resolve_target_extension_override(
            rv,
            override_,
            property_name,
            has_read,
            has_write,
        );
    }

    if let Some(super_) = rv.ast.cast::<SuperExpression>(target) {
        return resolve_target_super_expression(
            rv,
            node.upcast(),
            super_,
            property_name,
            has_read,
            has_write,
        );
    }

    let is_cascaded = rv.ast[node].target.is_none();
    let is_null_aware = property_access_is_null_aware(rv, node);
    resolve(
        rv,
        node.upcast(),
        target,
        is_cascaded,
        is_null_aware,
        property_name,
        has_read,
        has_write,
        original_node,
    )
}

/// Dart `PropertyElementResolver.resolveSimpleIdentifier(node:, hasRead:,
/// hasWrite:)`.
pub fn resolve_simple_identifier(
    rv: &mut ResolverVisitor<'_>,
    node: Id<SimpleIdentifier>,
    has_read: bool,
    has_write: bool,
) -> PropertyElementResolverResult {
    if let Some(ancestor_cascade) = ancestor_cascade(rv, node) {
        let target = rv.ast[ancestor_cascade].target;
        let is_null_aware = cascade_is_null_aware(rv, ancestor_cascade);
        return resolve(
            rv,
            node.upcast(),
            target,
            true,
            is_null_aware,
            node,
            has_read,
            has_write,
            None,
        );
    }

    let scope_lookup_result = rv
        .rt
        .scope_lookup_result
        .get(node)
        .copied()
        .unwrap_or_default();
    // Dart `reportDeprecatedExportUse(...)` (wave D).

    let mut read_element_requested = None;
    let mut get_type = None;
    if has_read {
        let ctx = rv.ctx;
        let read_lookup = match crate::lexical_lookup::resolve_getter(&ctx, scope_lookup_result) {
            Some(r) => Some(r),
            None => crate::this_lookup::lookup_getter(rv, node),
        };
        if let Some(call_function_type) = read_lookup.and_then(|r| r.call_function_type) {
            return PropertyElementResolverResult {
                function_type_call_type: Some(call_function_type),
                ..Default::default()
            };
        }
        if let Some(record_field) = read_lookup.and_then(|r| r.record_field) {
            return PropertyElementResolverResult {
                record_field: Some(record_field),
                ..Default::default()
            };
        }
        read_element_requested = read_lookup.and_then(|r| r.requested);
        if let Some(requested) = read_element_requested
            && member::base_element(&ctx, requested).is::<dartr_element::PropertyAccessorElement>()
            && !member::is_static(&ctx, requested)
        {
            let unpromoted_type = member::return_type(&ctx, requested);
            let name = ctx.name(rv.lexeme(rv.ast[node].token));
            if let Some(flow) = rv.flow_analysis.flow.as_mut() {
                let (promoted, info) = flow.property_get(
                    PropertyTarget::This,
                    name,
                    Some(requested),
                    SharedTypeView::new(unpromoted_type),
                );
                rv.flow_analysis.store_expression_info(node.upcast(), info);
                get_type = promoted.map(|t| t.unwrap_type_view());
            }
            get_type = get_type.or(Some(unpromoted_type));
        }
        rv.check_read_of_not_assigned_local_variable(node, read_element_requested);
    }

    let mut write_element_requested = None;
    let mut write_element_recovery = None;
    if has_write {
        let ctx = rv.ctx;
        let write_lookup = match crate::lexical_lookup::resolve_setter(&ctx, scope_lookup_result) {
            Some(r) => Some(r),
            None => crate::this_lookup::lookup_setter(rv, node),
        };
        write_element_requested = write_lookup.and_then(|r| r.requested);
        write_element_recovery = write_lookup.and_then(|r| r.recovery);

        crate::assignment_expression_resolver::verify_assignment(
            rv,
            node,
            write_element_requested,
            write_element_recovery,
            None,
        );
    }

    PropertyElementResolverResult {
        read_element_requested,
        read_element_recovery: None,
        write_element_requested,
        write_element_recovery,
        get_type,
        ..Default::default()
    }
}

/// Dart `_checkForStaticAccessToInstanceMember`: if [element] is not
/// static, reports the error on [identifier]. Returns whether an error was
/// reported.
fn check_for_static_access_to_instance_member(
    rv: &mut ResolverVisitor<'_>,
    identifier: Id<SimpleIdentifier>,
    element: ElemRef,
) -> bool {
    if is_static(rv, element) {
        return false;
    }
    let name = identifier_name(rv, identifier);
    let d = rv.at(diag::static_access_to_instance_member(&name), identifier);
    rv.report(d);
    true
}

/// Dart `_checkForStaticMember(target, propertyName, element)`.
fn check_for_static_member(
    rv: &mut ResolverVisitor<'_>,
    target: Id<Expression>,
    property_name: Id<SimpleIdentifier>,
    element: Option<ElemRef>,
) {
    let Some(element) = element else {
        return;
    };
    let ctx = rv.ctx;
    let base = member::base_element(&ctx, element);
    // Dart `element is ExecutableElement && element.isStatic`.
    if !base.is::<dartr_element::ExecutableElement>() || !is_static(rv, element) {
        return;
    }
    if rv.ast.is::<ExtensionOverride>(target) {
        let d = rv.at(
            diag::extension_override_access_to_static_member(),
            property_name,
        );
        rv.report(d);
        return;
    }
    let name = identifier_name(rv, property_name);
    let kind = element_kind_display_name(base);
    let Some(enclosing) = member::enclosing_element(&ctx, element) else {
        return;
    };
    let enclosing_name = ctx.element_name(enclosing);
    if enclosing.is::<ExtensionElement>() && enclosing_name.is_none() {
        let d = rv.at(
            diag::instance_access_to_static_member_of_unnamed_extension(&name, kind),
            property_name,
        );
        rv.report(d);
    } else {
        let enclosing_kind = element_kind_display_name(enclosing);
        let d = rv.at(
            diag::instance_access_to_static_member(
                &name,
                kind,
                enclosing_name.unwrap_or(""),
                enclosing_kind,
            ),
            property_name,
        );
        rv.report(d);
    }
}

/// Dart `_isAccessible(element)`.
fn is_accessible(rv: &ResolverVisitor<'_>, element: ElemRef) -> bool {
    member::is_accessible_in(&rv.ctx, element, rv.unit.library)
}

/// Dart `_reportUnresolvedIndex(node, locatableDiagnostic)`.
fn report_unresolved_index(
    rv: &mut ResolverVisitor<'_>,
    node: Id<IndexExpression>,
    d: LocatableDiagnostic,
) {
    let left_bracket = rv.ast.tokens.get(rv.ast[node].left_bracket);
    let right_bracket = rv.ast.tokens.get(rv.ast[node].right_bracket);
    let offset = left_bracket.offset;
    let length = right_bracket.end() - offset;
    rv.report(d.at_offset(offset as usize, length as usize));
}

/// Dart `_resolve(node:, target:, isCascaded:, isNullAware:, propertyName:,
/// hasRead:, hasWrite:, originalNode:)`.
#[allow(clippy::too_many_arguments)]
fn resolve(
    rv: &mut ResolverVisitor<'_>,
    node: Id<Expression>,
    target: Id<Expression>,
    is_cascaded: bool,
    is_null_aware: bool,
    property_name: Id<SimpleIdentifier>,
    has_read: bool,
    has_write: bool,
    original_node: Option<Id<PrefixedIdentifier>>,
) -> PropertyElementResolverResult {
    let ctx = rv.ctx;
    let ts = rv.type_system;
    //
    // If this property access is of the form 'C.m' where 'C' is a class,
    // then we don't call resolveProperty(...) which walks up the class
    // hierarchy, instead we just look for the member in the type only. This
    // does not apply to conditional property accesses (i.e. 'C?.m').
    //
    let target_element =
        if rv.ast.is::<SimpleIdentifier>(target) || rv.ast.is::<PrefixedIdentifier>(target) {
            rv.element(identifier_element_node(rv, target))
                .map(|e| member::base_element(&ctx, e))
        } else {
            None
        };
    if let Some(target_element) = target_element {
        if let Some(interface) = target_element.cast::<InterfaceElement>() {
            return resolve_target_interface_element(
                rv,
                interface,
                is_cascaded,
                property_name,
                has_read,
                has_write,
            );
        } else if target_element.tag() == Tag::TypeAlias {
            let aliased = match ctx.any(target_element) {
                dartr_element::AnyElement::TypeAlias(a) => a.aliased_type.get(),
                _ => None,
            };
            if let Some(interface) = aliased.and_then(|t| match ctx.ty(t) {
                TypeKind::Interface { element, .. } => Some(*element),
                _ => None,
            }) {
                return resolve_target_interface_element(
                    rv,
                    interface,
                    is_cascaded,
                    property_name,
                    has_read,
                    has_write,
                );
            }
        }

        //
        // If this property access is of the form 'E.m' where 'E' is an
        // extension, then look for the member in the extension. This does not
        // apply to conditional property accesses (i.e. 'C?.m').
        //
        if let Some(extension) = target_element.cast::<ExtensionElement>() {
            return resolve_target_extension_element(
                rv,
                extension,
                property_name,
                has_read,
                has_write,
            );
        }
    }

    let mut target_type = rv.static_type(target).unwrap_or(TypeId::DYNAMIC);
    let name = identifier_name(rv, property_name);

    if name == "call"
        && (matches!(ctx.ty(target_type), TypeKind::Function(_))
            || ctx.is_dart_core_function(target_type))
    {
        return PropertyElementResolverResult {
            function_type_call_type: Some(target_type),
            ..Default::default()
        };
    }

    if matches!(ctx.ty(target_type), TypeKind::Void) {
        let d = rv.at(diag::use_of_void_result(), property_name);
        rv.report(d);
        return PropertyElementResolverResult::default();
    }

    if is_null_aware {
        target_type = ts.promote_to_non_null(target_type);
    }

    if let Some(type_literal) = rv.ast.cast::<TypeLiteral>(target) {
        let named_type = rv.ast[type_literal].type_;
        if rv
            .tables
            .annotation_type
            .get(named_type)
            .is_some_and(|&t| matches!(ctx.ty(t), TypeKind::Function(_)))
        {
            // There is no possible resolution for a property access of a
            // function type literal (which can only be a type instantiation
            // of a type alias of a function type).
            let qualified_name = named_type_qualified_name(rv, named_type);
            let d = if has_read {
                diag::undefined_getter_on_function_type(&name, &qualified_name)
            } else {
                diag::undefined_setter_on_function_type(&name, &qualified_name)
            };
            let d = rv.at(d, property_name);
            rv.report(d);
            return PropertyElementResolverResult::default();
        }
    }

    let result = type_property_resolver::resolve(
        rv,
        PropertyQuery {
            receiver: Some(target),
            receiver_type: target_type,
            name: &name,
            has_read,
            has_write,
            property_error_entity: property_name.raw(),
            name_error_entity: property_name.raw(),
            parent_node: None,
        },
    );

    let mut get_type = None;
    if has_read {
        let unpromoted_type = match result.getter.map(|g| (g, member::base_element(&ctx, g))) {
            Some((g, base)) if base.tag() == Tag::Method => member::type_(&ctx, g),
            Some((g, base)) if base.is::<dartr_element::PropertyAccessorElement>() => {
                member::return_type(&ctx, g)
            }
            _ => result.record_field.map(|f| f.ty).unwrap_or(TypeId::DYNAMIC),
        };
        if rv.flow_analysis.flow.is_some() {
            let property_target = if is_cascaded {
                PropertyTarget::Cascade
            } else {
                PropertyTarget::Expression(rv.flow_analysis.get_expression_info(Some(target)))
            };
            let interned = ctx.name(&name);
            let flow = rv.flow_analysis.flow.as_mut().unwrap();
            let (promoted, info) = flow.property_get(
                property_target,
                interned,
                result.getter,
                SharedTypeView::new(unpromoted_type),
            );
            let info_node = original_node.map(|n| n.upcast()).unwrap_or(node);
            rv.flow_analysis.store_expression_info(info_node, info);
            get_type = promoted.map(|t| t.unwrap_type_view());
        }
        get_type = get_type.or(Some(unpromoted_type));

        check_for_static_member(rv, target, property_name, result.getter);
        if result.needs_getter_error {
            let d = diag::undefined_getter(&name, type_arg(&ctx, target_type));
            let d = rv.at(d, property_name);
            rv.report(d);
        }
    }

    if has_write {
        check_for_static_member(rv, target, property_name, result.setter);
        if result.needs_setter_error {
            let read_result = type_property_resolver::resolve(
                rv,
                PropertyQuery {
                    receiver: Some(target),
                    receiver_type: target_type,
                    name: &name,
                    has_read: true,
                    has_write: false,
                    property_error_entity: property_name.raw(),
                    name_error_entity: property_name.raw(),
                    parent_node: None,
                },
            );
            crate::assignment_expression_resolver::verify_assignment(
                rv,
                property_name,
                None,
                read_result.getter,
                Some(target_type),
            );
        }
    }

    PropertyElementResolverResult {
        read_element_requested: result.getter,
        read_element_recovery: result.setter,
        write_element_requested: result.setter,
        write_element_recovery: result.getter,
        at_dynamic_target: ts.is_dynamic_bounded(target_type),
        record_field: result.record_field,
        get_type,
        ..Default::default()
    }
}

/// The node that holds the element of the identifier [target] (Dart
/// `IdentifierImpl.element`: for a prefixed identifier, the element of its
/// identifier).
fn identifier_element_node(rv: &ResolverVisitor<'_>, target: Id<Expression>) -> NodeId {
    match rv.ast.cast::<PrefixedIdentifier>(target) {
        Some(p) => rv.ast[p].identifier.raw(),
        None => target.raw(),
    }
}

/// Dart `NamedType.qualifiedName`.
fn named_type_qualified_name(rv: &ResolverVisitor<'_>, node: Id<dartr_ast::NamedType>) -> String {
    let name = rv.lexeme(rv.ast[node].name).to_string();
    match rv.ast[node].import_prefix {
        Some(prefix) => format!("{}.{}", rv.lexeme(rv.ast[prefix].name), name),
        None => name,
    }
}

/// Dart `_resolveTargetExtensionElement`.
fn resolve_target_extension_element(
    rv: &mut ResolverVisitor<'_>,
    extension: EId<ExtensionElement>,
    property_name: Id<SimpleIdentifier>,
    has_read: bool,
    has_write: bool,
) -> PropertyElementResolverResult {
    let ctx = rv.ctx;
    let member_name = identifier_name(rv, property_name);
    let instance: EId<InstanceElement> = extension.upcast();
    let extension_name = element_name(rv, extension.raw());

    let mut read_element = None;
    let mut read_element_recovery = None;
    let mut get_type = None;
    if has_read {
        read_element = lookup::get_getter(&ctx, instance, &member_name)
            .map(|g| ElemRef::Base(g.raw()))
            .or_else(|| {
                lookup::get_method(&ctx, instance, &member_name).map(|m| ElemRef::Base(m.raw()))
            });

        match read_element {
            None => {
                // This method is only called for extension overrides, and
                // extension overrides can only refer to named extensions. So
                // it is safe to assume that `extension.name` is non-`null`.
                let d = rv.at(
                    diag::undefined_extension_getter(&member_name, &extension_name),
                    property_name,
                );
                rv.report(d);
            }
            Some(e) => {
                get_type = Some(member::return_type(&ctx, e));
                if check_for_static_access_to_instance_member(rv, property_name, e) {
                    read_element_recovery = read_element;
                    read_element = None;
                }
            }
        }
    }

    let mut write_element = None;
    let mut write_element_recovery = None;
    if has_write {
        write_element =
            lookup::get_setter(&ctx, instance, &member_name).map(|s| ElemRef::Base(s.raw()));
        match write_element {
            None => {
                let d = rv.at(
                    diag::undefined_extension_setter(&member_name, &extension_name),
                    property_name,
                );
                rv.report(d);
            }
            Some(e) => {
                if check_for_static_access_to_instance_member(rv, property_name, e) {
                    write_element_recovery = write_element;
                    write_element = None;
                }
            }
        }
    }

    PropertyElementResolverResult {
        read_element_requested: read_element,
        read_element_recovery,
        write_element_requested: write_element,
        write_element_recovery,
        get_type,
        ..Default::default()
    }
}

/// Dart `_resolveTargetExtensionOverride`.
fn resolve_target_extension_override(
    rv: &mut ResolverVisitor<'_>,
    target: Id<ExtensionOverride>,
    property_name: Id<SimpleIdentifier>,
    has_read: bool,
    has_write: bool,
) -> PropertyElementResolverResult {
    let ctx = rv.ctx;
    if rv
        .ast
        .parent(target)
        .is_some_and(|p| rv.ast.is::<CascadeExpression>(p))
    {
        // Report this error and recover by treating it like a non-cascade.
        let name = rv.ast[target].name;
        let d = rv.at_token(diag::extension_override_with_cascade(), name);
        rv.report(d);
    }

    let extension_name = rv
        .base_element(target)
        .map(|e| element_name(rv, e))
        .unwrap_or_default();
    let member_name = identifier_name(rv, property_name);

    let result = get_override_member(rv, target, &member_name);

    let mut read_element = None;
    let mut get_type = None;
    if has_read {
        read_element = result.getter;
        match read_element {
            None => {
                // This method is only called for extension overrides, and
                // extension overrides can only refer to named extensions. So
                // it is safe to assume that `element.name` is non-`null`.
                let d = rv.at(
                    diag::undefined_extension_getter(&member_name, &extension_name),
                    property_name,
                );
                rv.report(d);
            }
            Some(e) => get_type = Some(member::return_type(&ctx, e)),
        }
        check_for_static_member(rv, target.upcast(), property_name, read_element);
    }

    let mut write_element = None;
    if has_write {
        write_element = result.setter;
        if write_element.is_none() {
            let d = rv.at(
                diag::undefined_extension_setter(&member_name, &extension_name),
                property_name,
            );
            rv.report(d);
        }
        check_for_static_member(rv, target.upcast(), property_name, write_element);
    }

    PropertyElementResolverResult {
        read_element_requested: read_element,
        write_element_requested: write_element,
        get_type,
        ..Default::default()
    }
}

/// Dart `_resolveTargetInterfaceElement(typeReference:, isCascaded:,
/// propertyName:, hasRead:, hasWrite:)` (without the dot shorthand case).
fn resolve_target_interface_element(
    rv: &mut ResolverVisitor<'_>,
    type_reference: EId<InterfaceElement>,
    is_cascaded: bool,
    property_name: Id<SimpleIdentifier>,
    has_read: bool,
    has_write: bool,
) -> PropertyElementResolverResult {
    let ctx = rv.ctx;
    let mut type_reference = type_reference;
    if is_cascaded && let Some(type_element) = ctx.interface_element(ctx.tp.type_type()) {
        type_reference = type_element;
    }
    let instance: EId<InstanceElement> = type_reference.upcast();
    let name = identifier_name(rv, property_name);

    let mut read_element = None;
    let mut read_element_recovery = None;
    let mut get_type = None;
    if has_read {
        read_element = lookup::get_getter(&ctx, instance, &name).map(|g| ElemRef::Base(g.raw()));
        if read_element.is_some_and(|e| !is_accessible(rv, e)) {
            read_element = None;
        }

        if read_element.is_none() {
            read_element =
                lookup::get_method(&ctx, instance, &name).map(|m| ElemRef::Base(m.raw()));
            if read_element.is_some_and(|e| !is_accessible(rv, e)) {
                read_element = None;
            }
        }

        // Static extensions (`Feature.static_extensions`, an experiment)
        // are not ported.

        match read_element {
            Some(e) => {
                get_type = Some(member::return_type(&ctx, e));
                if check_for_static_access_to_instance_member(rv, property_name, e) {
                    read_element_recovery = read_element;
                    read_element = None;
                }
            }
            None => {
                let this_type = type_arg(&ctx, ctx.interface_this_type(type_reference));
                let d = if type_reference.raw().tag() == Tag::Enum {
                    diag::undefined_enum_constant(&name, this_type)
                } else {
                    diag::undefined_getter(&name, this_type)
                };
                let d = rv.at(d, property_name);
                rv.report(d);
            }
        }
    }

    let mut write_element = None;
    let mut write_element_recovery = None;
    if has_write {
        write_element = lookup::get_setter(&ctx, instance, &name).map(|s| ElemRef::Base(s.raw()));
        if let Some(e) = write_element {
            if !is_accessible(rv, e) {
                let d = rv.at(diag::private_setter(&name), property_name);
                rv.report(d);
            }
            if check_for_static_access_to_instance_member(rv, property_name, e) {
                write_element_recovery = write_element;
                write_element = None;
            }
        } else {
            // Recovery, try to use getter.
            write_element_recovery =
                lookup::get_getter(&ctx, instance, &name).map(|g| ElemRef::Base(g.raw()));
            let this_type = ctx.interface_this_type(type_reference);
            crate::assignment_expression_resolver::verify_assignment(
                rv,
                property_name,
                None,
                write_element_recovery,
                Some(this_type),
            );
        }
    }

    PropertyElementResolverResult {
        read_element_requested: read_element,
        read_element_recovery,
        write_element_requested: write_element,
        write_element_recovery,
        get_type,
        ..Default::default()
    }
}

/// Dart `_resolveTargetPrefixElement(target:, identifier:, hasRead:,
/// hasWrite:, forAnnotation:)`.
fn resolve_target_prefix_element(
    rv: &mut ResolverVisitor<'_>,
    target: EId<PrefixElement>,
    identifier: Id<SimpleIdentifier>,
    has_read: bool,
    has_write: bool,
    for_annotation: bool,
) -> PropertyElementResolverResult {
    let ctx = rv.ctx;
    let name = identifier_name(rv, identifier);
    // Dart `target.scope.lookup(identifier.name)`.
    let lookup_result = rv.unit.scopes.prefix_lookup(&ctx, target, &name);
    // Dart `reportDeprecatedExportUse(...)` (wave D).

    let read_element = lookup_result.getter.map(ElemRef::Base);
    let write_element = lookup_result.setter.map(ElemRef::Base);
    let mut get_type = None;
    if has_read
        && let Some(e) = read_element
        && member::base_element(&ctx, e).is::<dartr_element::PropertyAccessorElement>()
    {
        get_type = Some(member::return_type(&ctx, e));
    }

    if (has_read && read_element.is_none()) || (has_write && write_element.is_none()) {
        let prefix_name = element_name(rv, target.raw());
        // Dart `libraryFragment.shouldIgnoreUndefined(prefix:, name:)`: the
        // import state is not checked yet (wave D).
        if !for_annotation {
            let d = rv.at(
                diag::undefined_prefixed_name(&name, &prefix_name),
                identifier,
            );
            rv.report(d);
        }
    }

    PropertyElementResolverResult {
        read_element_requested: read_element,
        write_element_requested: write_element,
        get_type,
        ..Default::default()
    }
}

/// Dart `_resolveTargetSuperExpression(node:, target:, propertyName:,
/// hasRead:, hasWrite:)`.
fn resolve_target_super_expression(
    rv: &mut ResolverVisitor<'_>,
    node: Id<Expression>,
    target: Id<SuperExpression>,
    property_name: Id<SimpleIdentifier>,
    has_read: bool,
    has_write: bool,
) -> PropertyElementResolverResult {
    if !super_context_is_valid(rv, target) {
        return PropertyElementResolverResult::default();
    }
    let ctx = rv.ctx;
    let library = rv.unit.library;
    let target_type = rv.static_type(target);

    let mut read_element = None;
    let mut write_element = None;
    let mut get_type = None;

    if let Some(target_type) = target_type
        && let TypeKind::Interface { element, .. } = *ctx.ty(target_type)
    {
        let name = identifier_name(rv, property_name);
        let inheritance = InheritanceManager3::new(ctx);
        if has_read {
            let member_name = Name::for_library(&ctx, Some(library), &name);
            read_element = inheritance.get_member_with(
                element,
                member_name,
                GetMemberOptions {
                    for_super: true,
                    ..GetMemberOptions::default()
                },
            );

            if read_element.is_some() {
                check_for_static_member(rv, target.upcast(), property_name, read_element);
            } else {
                // We were not able to find the concrete dispatch target. But
                // we would like to give the user at least some resolution. So,
                // we retry simply looking for an inherited member.
                read_element = inheritance.get_inherited(element, member_name);
                if let Some(e) = read_element {
                    let kind = element_kind_display_name(member::base_element(&ctx, e));
                    let d = rv.at(
                        diag::abstract_super_member_reference(kind, &name),
                        property_name,
                    );
                    rv.report(d);
                } else {
                    let d = rv.at(
                        diag::undefined_super_getter(&name, type_arg(&ctx, target_type)),
                        property_name,
                    );
                    rv.report(d);
                }
            }
            let unpromoted_type = read_element
                .map(|e| member::return_type(&ctx, e))
                .unwrap_or(TypeId::DYNAMIC);
            if let Some(flow) = rv.flow_analysis.flow.as_mut() {
                let (promoted, info) = flow.property_get(
                    PropertyTarget::Super,
                    ctx.name(&name),
                    read_element,
                    SharedTypeView::new(unpromoted_type),
                );
                rv.flow_analysis.store_expression_info(node, info);
                get_type = promoted.map(|t| t.unwrap_type_view());
            }
            get_type = get_type.or(Some(unpromoted_type));
        }

        if has_write {
            write_element = lookup::type_look_up_setter(
                &ctx,
                target_type,
                &name,
                library,
                LookUpOptions {
                    concrete: true,
                    inherited: true,
                    recovery_static: false,
                },
            );

            if write_element.is_some() {
                check_for_static_member(rv, target.upcast(), property_name, write_element);
            } else {
                // We were not able to find the concrete dispatch target. But
                // we would like to give the user at least some resolution. So,
                // we retry without the "concrete" requirement.
                write_element = lookup::type_look_up_setter(
                    &ctx,
                    target_type,
                    &name,
                    library,
                    LookUpOptions {
                        concrete: false,
                        inherited: true,
                        recovery_static: false,
                    },
                );
                if let Some(e) = write_element {
                    let kind = element_kind_display_name(member::base_element(&ctx, e));
                    let d = rv.at(
                        diag::abstract_super_member_reference(kind, &name),
                        property_name,
                    );
                    rv.report(d);
                } else {
                    let d = rv.at(
                        diag::undefined_super_setter(&name, type_arg(&ctx, target_type)),
                        property_name,
                    );
                    rv.report(d);
                }
            }
        }
    }

    PropertyElementResolverResult {
        read_element_requested: read_element,
        write_element_requested: write_element,
        get_type,
        ..Default::default()
    }
}

/// Dart `_toIndexResult(result, atDynamicTarget:, hasRead:, hasWrite:)`.
fn to_index_result(
    rv: &ResolverVisitor<'_>,
    read_element: Option<ElemRef>,
    write_element: Option<ElemRef>,
    at_dynamic_target: bool,
    has_read: bool,
) -> PropertyElementResolverResult {
    let context_type = if has_read {
        read_element.and_then(|e| first_parameter_type(rv, e))
    } else {
        write_element.and_then(|e| first_parameter_type(rv, e))
    };

    PropertyElementResolverResult {
        at_dynamic_target,
        read_element_requested: read_element,
        write_element_requested: write_element,
        index_context_type: context_type,
        ..Default::default()
    }
}

/// Dart `ExtensionMemberResolver.getOverrideMember(node, name)`: the member
/// [name] of the extension of the override [node], substituted with the
/// type arguments of the override.
///
/// A copy of the C6 function (`extension_member_resolver.dart`) until that
/// unit lands; it reads the element and the type arguments that the
/// resolution of the override wrote.
pub fn get_override_member(
    rv: &ResolverVisitor<'_>,
    node: Id<ExtensionOverride>,
    name: &str,
) -> ExtensionResolutionResult {
    let ctx = rv.ctx;
    let Some(extension) = rv
        .base_element(node)
        .and_then(|e| e.cast::<ExtensionElement>())
    else {
        return ExtensionResolutionResult::default();
    };
    let instance: EId<InstanceElement> = extension.upcast();
    let (getter, setter) = if name == "[]" {
        (
            lookup::get_method(&ctx, instance, "[]").map(|m| m.raw()),
            lookup::get_method(&ctx, instance, "[]=").map(|m| m.raw()),
        )
    } else {
        (
            lookup::get_getter(&ctx, instance, name)
                .map(|g| g.raw())
                .or_else(|| lookup::get_method(&ctx, instance, name).map(|m| m.raw())),
            lookup::get_setter(&ctx, instance, name).map(|s| s.raw()),
        )
    };
    if getter.is_none() && setter.is_none() {
        return ExtensionResolutionResult::default();
    }
    let type_params = &ctx.instance(instance).type_params;
    let type_args: Vec<TypeId> = rv
        .tables
        .type_arg_types
        .get(node)
        .map(|l| ctx.list(*l).to_vec())
        .unwrap_or_default();
    let substitution = if type_args.len() == type_params.len() {
        MapSubstitution::from_pairs(type_params, &type_args)
    } else {
        MapSubstitution::from_pairs(&[], &[])
    };
    ExtensionResolutionResult {
        getter: getter.map(|e| member::substitute(&ctx, ElemRef::Base(e), &substitution)),
        setter: setter.map(|e| member::substitute(&ctx, ElemRef::Base(e), &substitution)),
        is_ambiguous: false,
    }
}

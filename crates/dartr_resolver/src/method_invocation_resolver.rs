// Dart source: pkg/analyzer/lib/src/dart/resolver/method_invocation_resolver.dart,
// pkg/analyzer/lib/src/generated/resolver.dart (visitMethodInvocation,
// _startNullAwareAccess), pkg/analyzer/lib/src/generated/super_context.dart

//! `MethodInvocationResolver`: resolves `MethodInvocation`s (top-level,
//! local, prefixed, static, instance, `super` and extension invocations),
//! and rewrites the invocations of getters, variables and record fields to
//! `FunctionExpressionInvocation`s.
//!
//! The dot shorthand parts (`resolveDotShorthand`) belong to the dot
//! shorthand unit; the building blocks are
//! [`crate::invocation_inference_helper::resolve_dot_shorthand_invocation`]
//! and [`crate::invocation_inferrer::InferrerKind::DotShorthandInvocation`].
//! Not ported: `reportDeprecatedExportUse` and the dead code verifier
//! (wave D), `shouldIgnoreUndefined` (see [`should_ignore_undefined`]).

use dartr_ast::{
    Annotation, AnonymousMethodBody, AnonymousMethodInvocation, ClassDeclaration, CompilationUnit,
    ConstructorDeclaration, ConstructorInitializer, EnumDeclaration, Expression,
    ExtensionDeclaration, ExtensionOverride, ExtensionTypeDeclaration, FieldDeclaration,
    FunctionExpressionInvocation, Id, Identifier, MethodDeclaration, MethodInvocation,
    MixinDeclaration, NodeId, PrefixedIdentifier, PropertyAccess, SimpleIdentifier,
    SuperExpression, TypeLiteral,
};
use dartr_diagnostics::diag;
use dartr_element::{
    AnyElement, EId, ElemRef, ElementId, ExecutableElement, ExtensionElement, InstanceElement,
    InterfaceElement, PrefixElement, PropertyAccessorElement, Tag, TypeId, TypeKind,
    VariableElement,
};
use dartr_flow::flow_analysis::{FlowAnalysis, PropertyTarget};
use dartr_flow::null_shorting::TypeAnalysisNullShortingInterface;
use dartr_flow::shared_type::{SharedTypeSchemaView, SharedTypeView};
use dartr_flow::type_analyzer::TypeAnalyzer;
use dartr_syntax::TokenType;
use dartr_typesystem::inheritance_manager3::{GetMemberOptions, InheritanceManager3, Name};
use dartr_typesystem::{TypeExt, lookup, member};

use crate::ast_ext;
use crate::function_expression_invocation_resolver;
use crate::invocation_inference_helper;
use crate::invocation_inferrer::{
    InferrerKind, InvocationInferrer, InvocationTarget, type_argument_types,
};
use crate::resolver::ResolverVisitor;
use crate::type_property_resolver::{self, PropertyQuery};

/// Dart `ResolverVisitor.visitMethodInvocation(node, contextType:)`.
pub fn visit_method_invocation(
    rv: &mut ResolverVisitor<'_>,
    node: Id<MethodInvocation>,
    context_type: TypeId,
) {
    // If the node is a dot shorthand, cache the context type for resolution.
    let is_dot_shorthand = rv.rt.is_dot_shorthand(node.raw());
    if is_dot_shorthand {
        rv.push_dot_shorthand_context(node.raw(), SharedTypeSchemaView::new(context_type));
    }

    rv.check_unreachable_node(node);
    let mut target = rv.ast[node].target;
    if let Some(t) = target {
        TypeAnalyzer::analyze_expression(
            rv,
            t,
            SharedTypeSchemaView::new(TypeId::UNKNOWN),
            true,
            false,
            false,
        );
        target = rv.pop_rewrite();
    }

    if method_invocation_is_null_aware(rv, node) {
        start_null_aware_access(rv, target);
        let method_name = rv.ast[node].method_name;
        crate::error::dead_code_verifier::visit_node(rv, method_name);
    }

    let type_arguments = rv.ast[node].type_arguments;
    rv.visit_opt(type_arguments);
    // Dart `elementResolver.visitMethodInvocation(node, ...)`.
    resolve(rv, node, context_type);

    let replacement = rv.insert_generic_function_instantiation(node.upcast(), context_type);
    let argument_list = rv.ast[node].argument_list;
    rv.check_for_argument_types_not_assignable_in_list(argument_list);
    rv.insert_implicit_call_reference(replacement, context_type);
    crate::error::dead_code_verifier::verify_method_invocation(rv, node);

    if is_dot_shorthand {
        rv.pop_dot_shorthand_context();
    }
}

/// Dart `MethodInvocationImpl.isNullAware`.
pub fn method_invocation_is_null_aware(
    rv: &ResolverVisitor<'_>,
    node: Id<MethodInvocation>,
) -> bool {
    let ast = &*rv.ast;
    if ast_ext::method_invocation_is_cascaded(ast, node) {
        // Dart `_ancestorCascade.isNullAware`: the first section starts with
        // `?..`.
        let mut current = ast.parent(node);
        while let Some(p) = current {
            if let Some(c) = ast.cast::<dartr_ast::CascadeExpression>(p) {
                let sections = ast.list(ast[c].cascade_sections);
                return sections.first().is_some_and(|&first| {
                    let begin = ast.begin_token(first.raw());
                    ast.tokens.ty(begin) == TokenType::QUESTION_PERIOD_PERIOD
                });
            }
            current = ast.parent(p);
        }
        return false;
    }
    ast[node].operator.is_some_and(|o| {
        let ty = ast.tokens.ty(o);
        ty == TokenType::QUESTION_PERIOD || ty == TokenType::QUESTION_PERIOD_PERIOD
    })
}

/// Dart `ResolverVisitor._startNullAwareAccess(target)`.
pub(crate) fn start_null_aware_access(
    rv: &mut ResolverVisitor<'_>,
    target: Option<Id<Expression>>,
) {
    if rv.flow_analysis.flow.is_none() {
        return;
    }
    // `null`: the target of a cascade; the null-aware cascade is handled by
    // `visitCascadeExpression`.
    let Some(target) = target else {
        return;
    };
    // `?.` to access static methods is equivalent to `.`.
    if let Some(s) = rv.ast.cast::<SimpleIdentifier>(target)
        && rv
            .base_element(s)
            .is_some_and(|e| e.is::<InterfaceElement>())
    {
        return;
    }
    let mut expression = target;
    if let Some(o) = rv.ast.cast::<ExtensionOverride>(target) {
        let arguments = rv.ast.list(rv.ast[rv.ast[o].argument_list].arguments);
        if arguments.len() == 1 && !rv.ast.is::<dartr_ast::NamedArgument>(arguments[0]) {
            expression = Id::from_raw(arguments[0].raw());
        }
    }
    let info = rv.flow_analysis.get_expression_info(Some(expression));
    let ty = rv.static_type(expression).unwrap_or(TypeId::DYNAMIC);
    let info = rv.start_null_shorting((), info, SharedTypeView::new(ty), None);
    rv.flow_analysis.store_expression_info(expression, info);
}

/// Dart `MethodInvocationResolver.resolve(node, whyNotPromotedArguments,
/// contextType:)`.
pub fn resolve(rv: &mut ResolverVisitor<'_>, node: Id<MethodInvocation>, context_type: TypeId) {
    let name_node = rv.ast[node].method_name;
    let name = rv.lexeme(rv.ast[name_node].token).to_string();
    let r = Resolver {
        node,
        name_node,
        name: &name,
        context_type,
    };
    r.resolve(rv);
}

/// The state of one `resolve` call (Dart `_invocation`, `_currentName`).
struct Resolver<'s> {
    node: Id<MethodInvocation>,
    name_node: Id<SimpleIdentifier>,
    name: &'s str,
    context_type: TypeId,
}

/// The result of a scope lookup as an element reference.
fn base(e: ElementId) -> ElemRef {
    ElemRef::Base(e)
}

impl Resolver<'_> {
    /// Dart `_currentName`.
    fn current_name(&self, rv: &ResolverVisitor<'_>) -> Name {
        Name::new(&rv.ctx, Some(rv.unit.library), self.name)
    }

    fn resolve(&self, rv: &mut ResolverVisitor<'_>) {
        let node = self.node;
        let Some(receiver) = ast_ext::method_invocation_real_target(rv.ast, node) else {
            return self.resolve_receiver_null(rv);
        };

        if let Some(s) = rv.ast.cast::<SimpleIdentifier>(receiver)
            && let Some(prefix) = rv.base_element(s).and_then(|e| e.cast::<PrefixElement>())
        {
            return self.resolve_receiver_prefix(rv, prefix);
        }

        let receiver_element = identifier_element(rv, receiver);
        if let Some(extension) = receiver_element.and_then(|e| e.cast::<ExtensionElement>()) {
            return self.resolve_extension_member(rv, extension);
        }

        if let Some(s) = rv.ast.cast::<SuperExpression>(receiver) {
            return self.resolve_receiver_super(rv, s);
        }

        if let Some(o) = rv.ast.cast::<ExtensionOverride>(receiver) {
            return self.resolve_extension_override(rv, o);
        }

        if let Some(e) = receiver_element {
            if let Some(interface) = e.cast::<InterfaceElement>() {
                return self.resolve_receiver_type_literal(rv, interface);
            } else if e.tag() == Tag::TypeAlias {
                let aliased = match rv.ctx.any(e) {
                    AnyElement::TypeAlias(a) => a.aliased_type.get(),
                    _ => None,
                };
                if let Some(element) = aliased.and_then(|t| match *rv.ctx.ty(t) {
                    TypeKind::Interface { element, .. } => Some(element),
                    _ => None,
                }) {
                    return self.resolve_receiver_type_literal(rv, element);
                }
            }
        }

        let mut receiver_type = rv.static_type(receiver).unwrap_or(TypeId::DYNAMIC);

        if rv.type_system.is_dynamic_bounded(receiver_type) {
            return self.resolve_receiver_dynamic_bounded(rv, receiver_type);
        }

        if let TypeKind::Never(_) = rv.ctx.ty(receiver_type) {
            return self.resolve_receiver_never(rv, receiver, receiver_type);
        }

        if let TypeKind::Void = rv.ctx.ty(receiver_type) {
            self.set_invalid_type_resolution(rv, true);
            report_use_of_void_type(rv, receiver.raw());
            return;
        }

        if method_invocation_is_null_aware(rv, node) {
            receiver_type = rv.type_system.promote_to_non_null(receiver_type);
        }

        if let Some(type_literal) = rv.ast.cast::<TypeLiteral>(receiver) {
            let named_type = rv.ast[type_literal].type_;
            let is_function_type = rv
                .tables
                .annotation_type
                .get(named_type)
                .is_some_and(|&t| matches!(rv.ctx.ty(t), TypeKind::Function(_)));
            if rv.ast[named_type].type_arguments.is_some() && is_function_type {
                // There is no possible resolution for a property access of a
                // function type literal (which can only be a type
                // instantiation of a type alias of a function type).
                let alias_name = rv.ast.qualified_name(named_type);
                let d = rv.at(
                    diag::undefined_method_on_function_type(self.name, &alias_name),
                    self.name_node,
                );
                rv.report(d);
                self.set_invalid_type_resolution(rv, true);
                return;
            }
        }

        self.resolve_receiver_type(rv, Some(receiver), receiver_type);
    }

    // ------------------------------------------------------------ reports

    /// Dart `_reportInstanceAccessToStaticMember(nameNode, element,
    /// nullReceiver)`.
    fn report_instance_access_to_static_member(
        &self,
        rv: &mut ResolverVisitor<'_>,
        element: ElemRef,
        null_receiver: bool,
    ) {
        let ctx = rv.ctx;
        let base = member::base_element(&ctx, element);
        let Some(enclosing) = ctx.element_data(base).and_then(|d| d.enclosing) else {
            return;
        };
        let d = if null_receiver {
            let name = dartr_element::diagnostics::element_display_string(&ctx, enclosing);
            if rv.enclosing_extension.is_some() {
                diag::unqualified_reference_to_static_member_of_extended_type(&name)
            } else {
                diag::unqualified_reference_to_non_local_static_member(&name)
            }
        } else if enclosing.tag() == Tag::Extension && ctx.element_name(enclosing).is_none() {
            diag::instance_access_to_static_member_of_unnamed_extension(
                self.name,
                base.kind().display_name(),
            )
        } else {
            let enclosing_kind = if enclosing.tag() == Tag::Mixin {
                "mixin"
            } else {
                enclosing.kind().display_name()
            };
            diag::instance_access_to_static_member(
                self.name,
                base.kind().display_name(),
                ctx.element_name(enclosing).unwrap_or(""),
                enclosing_kind,
            )
        };
        let d = rv.at(d, self.name_node);
        rv.report(d);
    }

    /// Dart `_reportInvocationOfNonFunction(methodName)`.
    fn report_invocation_of_non_function(&self, rv: &mut ResolverVisitor<'_>) {
        let d = rv.at(diag::invocation_of_non_function(self.name), self.name_node);
        rv.report(d);
    }

    /// Dart `_reportStaticAccessToInstanceMember(element, nameNode)`.
    fn report_static_access_to_instance_member(
        &self,
        rv: &mut ResolverVisitor<'_>,
        element: ElemRef,
    ) {
        if !member::is_static(&rv.ctx, element) {
            let d = rv.at(
                diag::static_access_to_instance_member(self.name),
                self.name_node,
            );
            rv.report(d);
        }
    }

    /// Dart `_reportUndefinedFunction(node, prefix:, name:, ...)`.
    fn report_undefined_function(&self, rv: &mut ResolverVisitor<'_>, prefix: Option<&str>) {
        self.set_invalid_type_resolution(rv, true);
        if should_ignore_undefined(rv, prefix, self.name) {
            return;
        }
        let d = rv.at(diag::undefined_function(self.name), self.name_node);
        rv.report(d);
    }

    /// Dart `_reportUndefinedMethodOrNew(receiver, methodName)`.
    fn report_undefined_method_or_new(
        &self,
        rv: &mut ResolverVisitor<'_>,
        receiver: EId<InterfaceElement>,
    ) {
        let receiver_name = rv
            .ctx
            .element_name(receiver.raw())
            .unwrap_or("")
            .to_string();
        if self.name == "new" {
            // Attempting to invoke the unnamed constructor via `C.new(`.
            if rv.is_constructor_tearoffs_enabled() {
                let d = rv.at(
                    diag::new_with_undefined_constructor_default(&receiver_name),
                    self.name_node,
                );
                rv.report(d);
            }
            // Otherwise the parser reports `experimentNotEnabled`.
        } else {
            let d = rv.at(
                diag::undefined_method(self.name, &receiver_name),
                self.name_node,
            );
            rv.report(d);
        }
    }

    // ------------------------------------------------------------ receivers

    /// Dart `_resolveExtensionMember(...)`: an invocation of a static member
    /// of a named extension (`E.foo()`).
    fn resolve_extension_member(
        &self,
        rv: &mut ResolverVisitor<'_>,
        extension: EId<ExtensionElement>,
    ) {
        let ctx = rv.ctx;
        let instance: EId<InstanceElement> = EId::from_raw(extension.raw());
        if let Some(getter) = lookup::get_getter(&ctx, instance, self.name) {
            let getter = base(getter.raw());
            rv.set_element(self.name_node, Some(getter));
            self.report_static_access_to_instance_member(rv, getter);
            let return_type = member::return_type(&ctx, getter);
            self.rewrite_as_function_expression_invocation(rv, return_type, false);
            return;
        }

        if let Some(method) = lookup::get_method(&ctx, instance, self.name) {
            let method = base(method.raw());
            rv.set_element(self.name_node, Some(method));
            self.report_static_access_to_instance_member(rv, method);
            let ty = member::type_(&ctx, method);
            self.set_resolution(rv, ty, Some(InvocationTarget::ExecutableElement(method)));
            return;
        }

        self.set_invalid_type_resolution(rv, true);
        // Only called for named extensions.
        let extension_name = ctx.element_name(extension.raw()).unwrap_or("").to_string();
        let d = rv.at(
            diag::undefined_extension_method(self.name, &extension_name),
            self.name_node,
        );
        rv.report(d);
    }

    /// Dart `_resolveExtensionOverride(...)`.
    fn resolve_extension_override(
        &self,
        rv: &mut ResolverVisitor<'_>,
        override_: Id<ExtensionOverride>,
    ) {
        let ctx = rv.ctx;
        let member =
            function_expression_invocation_resolver::get_override_member(rv, override_, self.name)
                .0;

        let Some(member) = member else {
            self.set_invalid_type_resolution(rv, true);
            // Extension overrides always refer to named extensions.
            let extension_name = rv
                .base_element(override_)
                .and_then(|e| ctx.element_name(e))
                .unwrap_or("")
                .to_string();
            let d = rv.at(
                diag::undefined_extension_method(self.name, &extension_name),
                self.name_node,
            );
            rv.report(d);
            return;
        };

        if member::is_static(&ctx, member) {
            let d = rv.at(
                diag::extension_override_access_to_static_member(),
                self.name_node,
            );
            rv.report(d);
        }

        if ast_ext::method_invocation_is_cascaded(rv.ast, self.node) {
            // Report this error and recover by treating it like a
            // non-cascade.
            let d = rv.at_token(
                diag::extension_override_with_cascade(),
                rv.ast[override_].name,
            );
            rv.report(d);
        }

        rv.set_element(self.name_node, Some(member));

        if member::base_element(&ctx, member).is::<PropertyAccessorElement>() {
            let return_type = member::return_type(&ctx, member);
            self.rewrite_as_function_expression_invocation(rv, return_type, false);
            return;
        }

        let ty = member::type_(&ctx, member);
        self.set_resolution(rv, ty, Some(InvocationTarget::ExecutableElement(member)));
    }

    /// Dart `_resolveReceiverDynamicBounded(node, receiverType, ...)`.
    fn resolve_receiver_dynamic_bounded(
        &self,
        rv: &mut ResolverVisitor<'_>,
        receiver_type: TypeId,
    ) {
        let ctx = rv.ctx;
        let node = self.node;
        let name_node = self.name_node;

        let object_element: EId<InstanceElement> = EId::from_raw(ctx.tp.object_element().raw());
        let target_element =
            lookup::get_method(&ctx, object_element, self.name).map(|m| base(m.raw()));

        let mut target = None;
        if let TypeKind::Invalid = ctx.ty(receiver_type) {
            rv.set_element(name_node, None);
            rv.set_static_type(name_node, TypeId::INVALID);
            rv.tables.invoke_type.insert(node, TypeId::INVALID);
            rv.record_static_type(node, TypeId::INVALID);
        } else if let Some(target_element) = target_element
            && !member::is_static(&ctx, target_element)
            && self.has_matching_object_method(rv, target_element)
        {
            rv.set_element(name_node, Some(target_element));
            target = Some(InvocationTarget::ExecutableElement(target_element));
            let ty = member::type_(&ctx, target_element);
            rv.set_static_type(name_node, ty);
            rv.tables.invoke_type.insert(node, ty);
            let return_type = member::return_type(&ctx, target_element);
            rv.record_static_type(node, return_type);
        } else {
            rv.set_element(name_node, None);
            rv.set_static_type(name_node, TypeId::DYNAMIC);
            rv.tables.invoke_type.insert(node, TypeId::DYNAMIC);
            rv.record_static_type(node, TypeId::DYNAMIC);
        }

        self.set_explicit_type_argument_types(rv);
        InvocationInferrer {
            kind: InferrerKind::MethodInvocation(node),
            argument_list: rv.ast[node].argument_list,
            context_type: self.context_type,
            target,
        }
        .resolve_invocation(rv);
    }

    /// Dart `_hasMatchingObjectMethod(target, arguments)`.
    fn has_matching_object_method(&self, rv: &ResolverVisitor<'_>, target: ElemRef) -> bool {
        let arguments = rv
            .ast
            .list(rv.ast[rv.ast[self.node].argument_list].arguments);
        let parameter_count = member::formal_parameters(&rv.ctx, target).len();
        arguments.len() == parameter_count
            && !arguments
                .iter()
                .any(|&a| rv.ast.is::<dartr_ast::NamedArgument>(a))
    }

    /// Dart `_resolveReceiverNever(...)`: an instance invocation on an
    /// expression of type `Never` or `Never?`.
    fn resolve_receiver_never(
        &self,
        rv: &mut ResolverVisitor<'_>,
        receiver: Id<Expression>,
        receiver_type: TypeId,
    ) {
        let ctx = rv.ctx;
        self.set_explicit_type_argument_types(rv);

        let nullability = ctx.nullability_suffix(receiver_type);
        if nullability == dartr_element::Nullability::Question {
            let object_element: EId<InstanceElement> = EId::from_raw(ctx.tp.object_element().raw());
            if let Some(object_member) = lookup::get_method(&ctx, object_element, self.name) {
                let object_member = base(object_member.raw());
                rv.set_element(self.name_node, Some(object_member));
                let ty = member::type_(&ctx, object_member);
                self.set_resolution(
                    rv,
                    ty,
                    Some(InvocationTarget::ExecutableElement(object_member)),
                );
            } else if method_invocation_is_null_aware(rv, self.node) {
                let never_nullable = ctx.never_type(dartr_element::Nullability::Question);
                self.resolve_unreachable_invocation(rv, receiver, never_nullable, false);
            } else {
                self.resolve_receiver_type(rv, Some(receiver), receiver_type);
            }
            return;
        }

        if nullability == dartr_element::Nullability::None {
            self.resolve_unreachable_invocation(rv, receiver, TypeId::NEVER, true);
        }
    }

    /// `resolveUnreachableInvocation` of Dart `_resolveReceiverNever`.
    fn resolve_unreachable_invocation(
        &self,
        rv: &mut ResolverVisitor<'_>,
        receiver: Id<Expression>,
        result_type: TypeId,
        report_receiver_of_type_never: bool,
    ) {
        let node = self.node;
        InvocationInferrer {
            kind: InferrerKind::MethodInvocation(node),
            argument_list: rv.ast[node].argument_list,
            context_type: self.context_type,
            target: None,
        }
        .resolve_invocation(rv);

        if report_receiver_of_type_never {
            let d = rv.at(diag::receiver_of_type_never(), receiver);
            rv.report(d);
        }

        rv.set_static_type(self.name_node, TypeId::DYNAMIC);
        rv.tables.invoke_type.insert(node, TypeId::DYNAMIC);
        rv.record_static_type(node, result_type);
    }

    /// Dart `_resolveReceiverNull(...)`: an invocation without a target
    /// (a local or top-level function, a method of `this`, ...).
    fn resolve_receiver_null(&self, rv: &mut ResolverVisitor<'_>) {
        let ctx = rv.ctx;
        let name_node = self.name_node;
        let scope_lookup_result = rv
            .rt
            .scope_lookup_result
            .get(name_node)
            .copied()
            .unwrap_or_default();
        // Dart `reportDeprecatedExportUseGetter(...)` (wave D).

        if let Some(mut element) = scope_lookup_result.getter {
            rv.set_element(name_node, Some(base(element)));
            if element.tag() == Tag::MultiplyDefined {
                match ctx.any(element) {
                    AnyElement::MultiplyDefined(m) if !m.conflicting_elements.is_empty() => {
                        element = m.conflicting_elements[0];
                    }
                    _ => {}
                }
            }
            if element.is::<PropertyAccessorElement>() {
                let return_type = member::return_type(&ctx, base(element));
                self.rewrite_as_function_expression_invocation(rv, return_type, false);
                return;
            }
            if element.is::<ExecutableElement>() {
                let ty = member::type_(&ctx, base(element));
                self.set_resolution(
                    rv,
                    ty,
                    Some(InvocationTarget::ExecutableElement(base(element))),
                );
                return;
            }
            if element.is::<VariableElement>() {
                rv.check_read_of_not_assigned_local_variable(name_node, Some(base(element)));
                let target_type =
                    rv.flow_analysis
                        .local_variable_type(&ctx, name_node.upcast(), element, true);
                self.rewrite_as_function_expression_invocation(rv, target_type, false);
                return;
            }
            if element.tag() == Tag::Prefix {
                self.set_invalid_type_resolution(rv, true);
                let d = rv.at(
                    diag::prefix_identifier_not_followed_by_dot(self.name),
                    name_node,
                );
                rv.report(d);
                return;
            }
            self.set_invalid_type_resolution(rv, false);
            self.report_invocation_of_non_function(rv);
            return;
        }

        let Some(receiver_type) = rv.effective_this_type() else {
            self.report_undefined_function(rv, None);
            return;
        };

        if let Some(element) = scope_lookup_result.setter {
            // If the scope lookup reveals a setter, but no getter, then the
            // getter may still be found up the inheritance chain (through
            // `_resolveReceiverType`). A setter that is top-level, declared
            // in an extension, or static is the accessed property
            // (erroneously).
            let enclosing = ctx.element_data(element).and_then(|d| d.enclosing);
            let no_getter_is_possible = enclosing
                .is_some_and(|e| e.tag() == Tag::Library || e.tag() == Tag::Extension)
                || (element.is::<ExecutableElement>() && member::is_static(&ctx, base(element)));
            if no_getter_is_possible {
                rv.set_element(name_node, Some(base(element)));
                self.set_invalid_type_resolution(rv, false);
                let receiver_type_name = match *ctx.ty(receiver_type) {
                    TypeKind::Interface { element, .. } => {
                        ctx.element_name(element.raw()).unwrap_or("").to_string()
                    }
                    TypeKind::Function(_) => "Function".to_string(),
                    _ => "<unknown>".to_string(),
                };
                let d = rv.at(
                    diag::undefined_method(self.name, &receiver_type_name),
                    name_node,
                );
                rv.report(d);
                return;
            }
        }

        self.resolve_receiver_type(rv, None, receiver_type);
    }

    /// Dart `_resolveReceiverPrefix(...)`: a top-level function invocation
    /// with a prefix.
    fn resolve_receiver_prefix(&self, rv: &mut ResolverVisitor<'_>, prefix: EId<PrefixElement>) {
        let ctx = rv.ctx;
        // Note: `prefix?.bar` is reported as an error in ElementResolver.
        // `loadLibrary` of a deferred import: the prefix scope answers the
        // `loadLibrary` function of the imported library.
        let scope_lookup_result = rv.unit.scopes.prefix_lookup(&ctx, prefix, self.name);
        // Dart `reportDeprecatedExportUseGetter(...)` (wave D).

        let mut element = scope_lookup_result.getter;
        rv.set_element(self.name_node, element.map(base));

        if let Some(e) = element
            && e.tag() == Tag::MultiplyDefined
            && let AnyElement::MultiplyDefined(m) = ctx.any(e)
            && let Some(&first) = m.conflicting_elements.first()
        {
            element = Some(first);
        }

        if let Some(e) = element {
            if e.is::<PropertyAccessorElement>() {
                let return_type = member::return_type(&ctx, base(e));
                self.rewrite_as_function_expression_invocation(rv, return_type, false);
                return;
            }
            if e.is::<ExecutableElement>() {
                let ty = member::type_(&ctx, base(e));
                self.set_resolution(rv, ty, Some(InvocationTarget::ExecutableElement(base(e))));
                return;
            }
        }

        let prefix_name = ctx.element_name(prefix.raw()).map(|s| s.to_string());
        self.report_undefined_function(rv, prefix_name.as_deref());
    }

    /// Dart `_resolveReceiverSuper(...)`: an instance invocation on `super`.
    fn resolve_receiver_super(&self, rv: &mut ResolverVisitor<'_>, receiver: Id<SuperExpression>) {
        let ctx = rv.ctx;
        let Some(enclosing_class) = rv.enclosing_class else {
            self.set_invalid_type_resolution(rv, true);
            return;
        };
        if super_context_of(rv, receiver) != SuperContext::Valid {
            self.set_invalid_type_resolution(rv, true);
            return;
        }

        let inheritance = InheritanceManager3::new(ctx);
        let name = self.current_name(rv);
        let target = inheritance.get_member_with(
            enclosing_class,
            name,
            GetMemberOptions {
                for_super: true,
                ..GetMemberOptions::default()
            },
        );

        // If there is that concrete dispatch target, then we are done.
        if let Some(target) = target {
            rv.set_element(self.name_node, Some(target));
            if member::base_element(&ctx, target).is::<PropertyAccessorElement>() {
                let return_type = member::return_type(&ctx, target);
                self.rewrite_as_function_expression_invocation(rv, return_type, true);
                return;
            }
            let ty = member::type_(&ctx, target);
            self.set_resolution(rv, ty, Some(InvocationTarget::ExecutableElement(target)));
            return;
        }

        // Otherwise, this is an error. But give the user at least some
        // resolution: the interface target.
        if let Some(target) = inheritance.get_inherited(enclosing_class, name) {
            rv.set_element(self.name_node, Some(target));
            let ty = member::type_(&ctx, target);
            self.set_resolution(rv, ty, Some(InvocationTarget::ExecutableElement(target)));
            let kind = member::base_element(&ctx, target).kind().display_name();
            let d = rv.at(
                diag::abstract_super_member_reference(kind, self.name),
                self.name_node,
            );
            rv.report(d);
            return;
        }

        // Nothing helps, there is no target at all.
        self.set_invalid_type_resolution(rv, true);
        let class_name = ctx
            .element_name(enclosing_class.raw())
            .unwrap_or("")
            .to_string();
        let d = rv.at(
            diag::undefined_super_method(self.name, &class_name),
            self.name_node,
        );
        rv.report(d);
    }

    /// Dart `_resolveReceiverType(...)`: an instance invocation on a
    /// receiver of [receiver_type] (`None` [receiver]: implicit `this`).
    fn resolve_receiver_type(
        &self,
        rv: &mut ResolverVisitor<'_>,
        receiver: Option<Id<Expression>>,
        receiver_type: TypeId,
    ) {
        let ctx = rv.ctx;
        let name_node = self.name_node;
        let result = type_property_resolver::resolve(
            rv,
            PropertyQuery {
                receiver,
                receiver_type,
                name: self.name,
                has_read: true,
                has_write: false,
                property_error_entity: name_node.raw(),
                name_error_entity: name_node.raw(),
                parent_node: None,
            },
        );

        if let Some(call_function_type) = result.call_function_type {
            self.set_resolution(
                rv,
                call_function_type,
                Some(InvocationTarget::FunctionTypedExpression(
                    call_function_type,
                )),
            );
            // Erase the resolution that `_setResolution()` sets.
            rv.set_element(name_node, None);
            rv.set_static_type(name_node, TypeId::DYNAMIC);
            return;
        }

        if ctx.is_dart_core_function(receiver_type) && self.name == "call" {
            self.set_resolution(rv, TypeId::DYNAMIC, None);
            rv.set_element(name_node, None);
            rv.set_static_type(name_node, TypeId::DYNAMIC);
            rv.tables.invoke_type.insert(self.node, TypeId::DYNAMIC);
            rv.set_static_type(self.node, TypeId::DYNAMIC);
            return;
        }

        if let Some(record_field) = result.record_field {
            self.rewrite_as_function_expression_invocation(rv, record_field.ty, false);
            return;
        }

        if let Some(target) = result.getter {
            rv.set_element(name_node, Some(target));

            if member::is_static(&ctx, target) {
                self.report_instance_access_to_static_member(rv, target, receiver.is_none());
            }

            if member::base_element(&ctx, target).is::<PropertyAccessorElement>() {
                let return_type = member::return_type(&ctx, target);
                self.rewrite_as_function_expression_invocation(rv, return_type, false);
                return;
            }
            let ty = member::type_(&ctx, target);
            self.set_resolution(rv, ty, Some(InvocationTarget::ExecutableElement(target)));
            return;
        }

        self.set_invalid_type_resolution(rv, true);

        if !result.needs_getter_error {
            return;
        }

        let receiver_class_name = match *ctx.ty(receiver_type) {
            TypeKind::Interface { element, .. } => match ctx.element_name(element.raw()) {
                Some(name) => name.to_string(),
                None => return,
            },
            TypeKind::Function(_) => "Function".to_string(),
            _ => "<unknown>".to_string(),
        };

        if !ast_ext::token_is_synthetic(rv.ast, rv.ast[name_node].token) {
            let d = rv.at(
                diag::undefined_method(self.name, &receiver_class_name),
                name_node,
            );
            rv.report(d);
        }
    }

    /// Dart `_resolveReceiverTypeLiteral(...)`: an invocation with a type
    /// literal target (a static method, or a method of `Type` in a
    /// cascade).
    fn resolve_receiver_type_literal(
        &self,
        rv: &mut ResolverVisitor<'_>,
        mut receiver: EId<InterfaceElement>,
    ) {
        let ctx = rv.ctx;
        if ast_ext::method_invocation_is_cascaded(rv.ast, self.node)
            && let Some(type_element) = ctx.interface_element(ctx.tp.type_type())
        {
            receiver = type_element;
        }

        if let Some(element) = self.resolve_element(rv, receiver) {
            if element.is::<ExecutableElement>() {
                let element = base(element);
                rv.set_element(self.name_node, Some(element));
                if member::base_element(&ctx, element).is::<PropertyAccessorElement>() {
                    let return_type = member::return_type(&ctx, element);
                    self.rewrite_as_function_expression_invocation(rv, return_type, false);
                    return;
                }
                let ty = member::type_(&ctx, element);
                self.set_resolution(rv, ty, Some(InvocationTarget::ExecutableElement(element)));
            } else {
                self.set_invalid_type_resolution(rv, false);
                self.report_invocation_of_non_function(rv);
            }
            return;
        }

        self.set_invalid_type_resolution(rv, true);
        self.report_undefined_method_or_new(rv, receiver);
    }

    /// Dart `_resolveElement(classElement, propertyName)`: the getter (the
    /// setter in a setter context) or method [name] of [class_element].
    fn resolve_element(
        &self,
        rv: &ResolverVisitor<'_>,
        class_element: EId<InterfaceElement>,
    ) -> Option<ElementId> {
        let ctx = rv.ctx;
        let instance: EId<InstanceElement> = EId::from_raw(class_element.raw());
        let mut element = None;
        if ast_ext::simple_identifier_in_setter_context(rv.ast, self.name_node) {
            element = lookup::get_setter(&ctx, instance, self.name).map(|e| e.raw());
        }
        let element = element
            .or_else(|| lookup::get_getter(&ctx, instance, self.name).map(|e| e.raw()))
            .or_else(|| lookup::get_method(&ctx, instance, self.name).map(|e| e.raw()))?;
        member::is_accessible_in(&ctx, base(element), rv.unit.library).then_some(element)
    }

    // ------------------------------------------------------------ rewrite

    /// Dart `_rewriteAsFunctionExpressionInvocation(...)`: [node] does not
    /// invoke a method, but the result of a getter, a variable or a record
    /// field (or the `call` method of an interface type), so it is a
    /// `FunctionExpressionInvocation`.
    fn rewrite_as_function_expression_invocation(
        &self,
        rv: &mut ResolverVisitor<'_>,
        getter_return_type: TypeId,
        is_super_access: bool,
    ) -> Id<FunctionExpressionInvocation> {
        let _ = is_super_access;
        let ctx = rv.ctx;
        let node = self.node;
        let method_name = self.name_node;
        let target = rv.ast[node].target;
        let operator = rv.ast[node].operator;
        let is_cascaded = ast_ext::method_invocation_is_cascaded(rv.ast, node);
        let mut target_type = getter_return_type;
        let property_name = ctx.name(self.name);

        let function_expression: Id<Expression>;
        match target {
            None => {
                function_expression = match (is_cascaded, operator) {
                    (true, Some(operator)) => rv
                        .ast
                        .add(PropertyAccess {
                            target: None,
                            operator,
                            property_name: method_name,
                        })
                        .upcast(),
                    _ => method_name.upcast(),
                };

                let element = rv.element(method_name);
                let is_instance_member = element.is_some_and(|e| {
                    let b = member::base_element(&ctx, e);
                    b.is::<ExecutableElement>()
                        && ctx
                            .element_data(b)
                            .and_then(|d| d.enclosing)
                            .is_some_and(|en| en.is::<InstanceElement>())
                        && !member::is_static(&ctx, e)
                });
                if is_instance_member && let Some(flow) = rv.flow_analysis.flow.as_mut() {
                    let property_target = if is_cascaded {
                        PropertyTarget::Cascade
                    } else {
                        PropertyTarget::This
                    };
                    let (wrapped_promoted_type, expression_info) = flow.property_get(
                        property_target,
                        property_name,
                        element,
                        SharedTypeView::new(getter_return_type),
                    );
                    rv.flow_analysis
                        .store_expression_info(function_expression, expression_info);
                    if let Some(t) = wrapped_promoted_type {
                        target_type = t.unwrap_type_view();
                    }
                }
            }
            Some(target) => {
                let is_prefix = rv
                    .ast
                    .cast::<SimpleIdentifier>(target)
                    .is_some_and(|s| rv.base_element(s).is_some_and(|e| e.tag() == Tag::Prefix));
                let operator = operator.expect("operator of a method invocation with a target");
                function_expression = if is_prefix {
                    rv.ast
                        .add(PrefixedIdentifier {
                            prefix: Id::from_raw(target.raw()),
                            period: operator,
                            identifier: method_name,
                        })
                        .upcast()
                } else {
                    rv.ast
                        .add(PropertyAccess {
                            target: Some(target),
                            operator,
                            property_name: method_name,
                        })
                        .upcast()
                };
                if rv.flow_analysis.flow.is_some() {
                    let property_target = if rv.ast.is::<SuperExpression>(target) {
                        PropertyTarget::Super
                    } else {
                        PropertyTarget::Expression(
                            rv.flow_analysis.get_expression_info(Some(target)),
                        )
                    };
                    let element = rv.element(method_name);
                    let flow = rv.flow_analysis.flow.as_mut().expect("flow");
                    let (wrapped_promoted_type, expression_info) = flow.property_get(
                        property_target,
                        property_name,
                        element,
                        SharedTypeView::new(getter_return_type),
                    );
                    rv.flow_analysis
                        .store_expression_info(function_expression, expression_info);
                    if let Some(t) = wrapped_promoted_type {
                        target_type = t.unwrap_type_view();
                    }
                }
            }
        }
        rv.record_static_type(method_name, target_type);

        if function_expression != method_name.upcast() {
            rv.set_static_type(function_expression, target_type);
        }

        let (type_arguments, argument_list) =
            (rv.ast[node].type_arguments, rv.ast[node].argument_list);
        let invocation = rv.ast.add(FunctionExpressionInvocation {
            function: function_expression,
            type_arguments,
            argument_list,
        });
        rv.replace_expression(node.upcast(), invocation.upcast(), None);
        function_expression_invocation_resolver::resolve(rv, invocation, self.context_type);
        invocation
    }

    // ------------------------------------------------------------ results

    /// Dart `_setDynamicTypeResolution(node, setNameTypeToDynamic:, ...)`.
    fn set_dynamic_type_resolution(
        &self,
        rv: &mut ResolverVisitor<'_>,
        set_name_type_to_dynamic: bool,
    ) {
        if set_name_type_to_dynamic {
            rv.set_static_type(self.name_node, TypeId::DYNAMIC);
        }
        rv.tables.invoke_type.insert(self.node, TypeId::DYNAMIC);
        rv.set_static_type(self.node, TypeId::DYNAMIC);
        self.set_explicit_type_argument_types(rv);
        self.resolve_arguments_finish_inference(rv);
    }

    /// Dart `_resolveArguments_finishInference(node, ...)`.
    fn resolve_arguments_finish_inference(&self, rv: &mut ResolverVisitor<'_>) {
        let static_type = InvocationInferrer {
            kind: InferrerKind::MethodInvocation(self.node),
            argument_list: rv.ast[self.node].argument_list,
            context_type: self.context_type,
            target: None,
        }
        .resolve_invocation(rv);
        rv.record_static_type(self.node, static_type);
    }

    /// Dart `_setExplicitTypeArgumentTypes()`: the explicit type argument
    /// types, or empty.
    fn set_explicit_type_argument_types(&self, rv: &mut ResolverVisitor<'_>) {
        let types = match rv.ast[self.node].type_arguments {
            Some(list) => type_argument_types(rv, list),
            None => Vec::new(),
        };
        let list = rv.ctx.intern_list(&types);
        rv.tables.type_arg_types.insert(self.node, list);
    }

    /// Dart `_setInvalidTypeResolution(node, setNameTypeToDynamic:, ...)`.
    fn set_invalid_type_resolution(
        &self,
        rv: &mut ResolverVisitor<'_>,
        set_name_type_to_dynamic: bool,
    ) {
        if set_name_type_to_dynamic {
            rv.set_static_type(self.name_node, TypeId::INVALID);
        }
        self.set_explicit_type_argument_types(rv);
        self.resolve_arguments_finish_inference(rv);
        rv.tables.invoke_type.insert(self.node, TypeId::INVALID);
        rv.set_static_type(self.node, TypeId::INVALID);
    }

    /// Dart `_setResolution(node, type, whyNotPromotedArguments,
    /// contextType:, target:)`.
    fn set_resolution(
        &self,
        rv: &mut ResolverVisitor<'_>,
        ty: TypeId,
        target: Option<InvocationTarget>,
    ) {
        // Dart: "We need this for StaticTypeAnalyzer to run inference."
        rv.set_static_type(self.name_node, ty);

        if ty == TypeId::DYNAMIC || rv.ctx.is_dart_core_function(ty) {
            self.set_dynamic_type_resolution(rv, false);
            return;
        }

        match rv.ctx.ty(ty) {
            TypeKind::Function(_) => {
                invocation_inference_helper::resolve_method_invocation(
                    rv,
                    self.node,
                    self.context_type,
                    target,
                );
            }
            TypeKind::Void => {
                self.set_invalid_type_resolution(rv, true);
                report_use_of_void_type(rv, self.name_node.raw());
            }
            _ => {
                self.set_invalid_type_resolution(rv, false);
                self.report_invocation_of_non_function(rv);
            }
        }
    }
}

/// Dart `Identifier.element` of [expression] (the element of the
/// identifier of a `PrefixedIdentifier`), if it is an identifier.
fn identifier_element(rv: &ResolverVisitor<'_>, expression: Id<Expression>) -> Option<ElementId> {
    if !rv.ast.is::<Identifier>(expression) {
        return None;
    }
    if let Some(p) = rv.ast.cast::<PrefixedIdentifier>(expression) {
        return rv
            .base_element(p)
            .or_else(|| rv.base_element(rv.ast[p].identifier));
    }
    rv.base_element(expression)
}

/// Dart `_reportUseOfVoidType(errorNode)`.
fn report_use_of_void_type(rv: &mut ResolverVisitor<'_>, error_node: NodeId) {
    let d = rv.at(diag::use_of_void_result(), error_node);
    rv.report(d);
}

/// Dart `libraryFragment.shouldIgnoreUndefined(prefix:, name:)`: the name
/// may come from an import of a library that does not exist. STUB: the
/// import state is not checked yet (as in `simple_identifier_resolver`);
/// nothing is ignored.
fn should_ignore_undefined(rv: &ResolverVisitor<'_>, prefix: Option<&str>, name: &str) -> bool {
    let _ = (rv, prefix, name);
    false
}

// ------------------------------------------------------------------ super context

/// Dart `SuperContext`: the context of a `super` expression.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SuperContext {
    Annotation,
    Extension,
    ExtensionType,
    Static,
    AnonymousMethod,
    Valid,
}

/// Dart `SuperContext.of(expression)`.
pub(crate) fn super_context_of(
    rv: &ResolverVisitor<'_>,
    expression: Id<SuperExpression>,
) -> SuperContext {
    let ast = &*rv.ast;
    let mut current: Option<NodeId> = Some(expression.raw());
    while let Some(node) = current {
        if ast.is::<Annotation>(node) {
            return SuperContext::Annotation;
        } else if ast.is::<AnonymousMethodBody>(node)
            && ast
                .parent(node)
                .and_then(|p| ast.cast::<AnonymousMethodInvocation>(p))
                .is_some_and(|p| ast[p].parameters.is_none())
        {
            return SuperContext::AnonymousMethod;
        } else if ast.is::<ClassDeclaration>(node) {
            return SuperContext::Valid;
        } else if ast.is::<CompilationUnit>(node) {
            return SuperContext::Static;
        } else if let Some(c) = ast.cast::<ConstructorDeclaration>(node) {
            if ast[c].factory_keyword.is_some() {
                return SuperContext::Static;
            }
        } else if ast.is::<ConstructorInitializer>(node) {
            return SuperContext::Static;
        } else if ast.is::<EnumDeclaration>(node) {
            return SuperContext::Valid;
        } else if ast.is::<ExtensionDeclaration>(node) {
            return SuperContext::Extension;
        } else if ast.is::<ExtensionTypeDeclaration>(node) {
            return SuperContext::ExtensionType;
        } else if let Some(f) = ast.cast::<FieldDeclaration>(node) {
            if ast[f].static_keyword.is_some() {
                return SuperContext::Static;
            }
            if ast[ast[f].fields].late_keyword.is_none() {
                return SuperContext::Static;
            }
        } else if let Some(m) = ast.cast::<MethodDeclaration>(node) {
            if ast_ext::is_keyword(ast, ast[m].modifier_keyword, "static") {
                return SuperContext::Static;
            }
        } else if ast.is::<MixinDeclaration>(node) {
            return SuperContext::Valid;
        }
        current = ast.parent(node);
    }
    SuperContext::Static
}

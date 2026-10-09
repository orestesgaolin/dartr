// Dart source: pkg/analyzer/lib/src/generated/error_verifier.dart
// (ErrorVerifier section: constructors, initializers, fields and variables (D6))

//! An `ErrorVerifier` section (see the module documentation of
//! [`super`]). The `visit_x` methods are the Dart `visitX` overrides; the
//! body `self.visit_children(node)` is Dart `super.visitX(node)`.

use dartr_ast::Entity;
use dartr_ast::*;
use dartr_diagnostics::{LocatableDiagnostic, diag};
use dartr_element::diagnostics::{element_arg, type_arg};
use dartr_element::{
    ConstructorElement, EId, ElemRef, ElementFlags, ElementId, FormalParameterElement,
    FragmentFlags, FragmentId, InterfaceElement, Tag, TypeId, TypeKind, TypeParameterElement,
    Variance,
};
use dartr_flow::shared_type::Variance as SharedVariance;
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_syntax::TokenId;
use dartr_typesystem::member;
use indexmap::IndexSet;

use super::{EnclosingExecutableContext, ErrorVerifier, ThisContext};
use crate::ast_ext::formal_parameter_parts;
use crate::element_ext::{first_fragment_flags, variable_type};
use crate::error::super_formal_parameters_verifier::verify_super_formal_parameters;
use crate::error::{const_argument_verifier, required_parameters_verifier, use_result_verifier};
use crate::error_detection_helpers::{ErrorDetectionHelpers, NonAssignabilityReporter};

/// Dart `SourceRange`: an offset and a length.
#[derive(Clone, Copy, Debug)]
struct SourceRange {
    offset: u32,
    length: u32,
}

impl ErrorVerifier<'_> {
    /// Dart `visitConstructorDeclaration`.
    pub(super) fn visit_constructor_declaration(&mut self, node: Id<ConstructorDeclaration>) {
        let Some(fragment) = self.declared_fragment_of(node) else {
            self.visit_children(node);
            return;
        };
        let Some(element) = self.fragment_element(fragment) else {
            self.visit_children(node);
            return;
        };
        let n = &self.ast[node];
        let (augment_keyword, parameters) = (n.augment_keyword, n.parameters);

        self.check_augmentation_without_declaration(augment_keyword, fragment);
        self.check_for_constructor_augmentation_modifier_mismatch(node, fragment);
        self.check_for_augmentation_formal_parameters(fragment, parameters);

        if self.fragment_has(fragment, FragmentFlags::FRAGMENT_IS_AUGMENTATION)
            && self.fragment_has(fragment, FragmentFlags::FRAGMENT_IS_COMPLETE)
            && let Some(augment_keyword) = augment_keyword
            && self.nearest_preceding_complete_fragment(fragment).is_some()
        {
            self.report_at_token(diag::constructor_already_complete(), augment_keyword);
        }

        self.check_for_factory_body_completeness(node);

        let context = EnclosingExecutableContext::new(
            &self.ctx,
            Some(element),
            self.fragment_has(fragment, FragmentFlags::EXECUTABLE_FRAGMENT_IS_ASYNCHRONOUS),
            self.fragment_has(fragment, FragmentFlags::EXECUTABLE_FRAGMENT_IS_GENERATOR),
        );
        self.with_enclosing_executable(context, |this| {
            let n = &this.ast[node];
            let (factory_keyword, const_keyword, external_keyword) =
                (n.factory_keyword, n.const_keyword, n.external_keyword);
            let (initializers, redirected, body) =
                (n.initializers, n.redirected_constructor, n.body);
            this.check_for_non_const_generative_enum_constructor(node);
            this.check_for_invalid_modifier_on_body(body);
            let error_range = this.constructor_error_range(node);
            if !this.check_for_const_constructor_with_non_const_super(
                factory_keyword,
                Some(initializers),
                element,
                error_range,
            ) {
                this.check_for_const_constructor_with_non_final_field(element, error_range);
                let is_redirecting = redirected.is_some()
                    || this
                        .ast
                        .list_raw(initializers)
                        .iter()
                        .any(|&i| this.ast.is::<RedirectingConstructorInvocation>(i));
                this.validate_constructor_body_allowed(
                    element,
                    const_keyword,
                    external_keyword,
                    is_redirecting,
                    body,
                    false,
                );
            }
            this.check_for_redirecting_constructor_error_codes(node);
            this.check_for_conflicting_initializer_error_codes(node);
            this.check_for_recursive_constructor_redirect(node, element);
            if !this.check_for_recursive_factory_redirect(node, element) {
                this.check_for_all_redirect_constructor_error_codes(node);
            }
            this.check_for_undefined_constructor_in_initializer_implicit_constructor(node);
            this.check_for_return_in_generative_constructor(node);
            this.check_for_non_redirecting_generative_constructor_with_primary(node);

            // Dart `node.visitChildrenWithHooks(this, visitInitializers:,
            // visitBody:)`.
            let body_context = if factory_keyword.is_some() {
                ThisContext::FactoryConstructorBody
            } else {
                ThisContext::GenerativeConstructorBody
            };
            this.visit_children_with_hooks(node.raw(), initializers, body, body_context);
        });
    }

    /// Dart `visitConstructorFieldInitializer`.
    pub(super) fn visit_constructor_field_initializer(
        &mut self,
        node: Id<ConstructorFieldInitializer>,
    ) {
        let field_name = self.ast[node].field_name;
        let element = self.element(field_name);
        self.check_for_invalid_field(node, field_name, element);
        if let Some(element) = element {
            let base = member::base_element(&self.ctx, element);
            if base.tag() == Tag::Field {
                let token = self.ast[field_name].token;
                self.check_for_abstract_or_external_field_constructor_initializer(
                    Entity::Token(token),
                    base,
                );
            }
        }
        self.visit_children(node);
    }

    /// Dart `visitDotShorthandConstructorInvocation`.
    pub(super) fn visit_dot_shorthand_constructor_invocation(
        &mut self,
        node: Id<DotShorthandConstructorInvocation>,
    ) {
        let constructor_name = self.ast[node].constructor_name;
        let constructor_element = self.element(constructor_name);
        let is_constructor = match constructor_element {
            None => true,
            Some(e) => member::base_element(&self.ctx, e).tag() == Tag::Constructor,
        };
        if is_constructor && !self.is_in_unresolved_annotation(node.raw()) {
            if self.dot_shorthand_constructor_invocation_is_const(node) {
                let const_keyword = self.ast[node].const_keyword;
                self.check_for_const_with_non_const(node.raw(), constructor_element, const_keyword);
            }
            self.check_for_invalid_generative_constructor_reference(
                constructor_name.raw(),
                constructor_element,
            );
        }
        required_parameters_verifier::visit_dot_shorthand_constructor_invocation(self, node);
        self.visit_children(node);
    }

    /// Dart `visitFieldDeclaration`.
    pub(super) fn visit_field_declaration(&mut self, node: Id<FieldDeclaration>) {
        let n = &self.ast[node];
        let (augment_keyword, static_keyword, abstract_keyword, fields) = (
            n.augment_keyword,
            n.static_keyword,
            n.abstract_keyword,
            n.fields,
        );
        let is_static = static_keyword.is_some();
        let variables: Vec<Id<VariableDeclaration>> =
            self.ast.list(self.ast[fields].variables).to_vec();
        let type_node = self.ast[fields].type_;

        for &variable in &variables {
            if let Some(declared_fragment) = self.declared_fragment_of(variable) {
                self.check_variable_augmentation(
                    augment_keyword,
                    variable,
                    declared_fragment,
                    type_node,
                );
            }
        }

        if is_static && abstract_keyword.is_some() {
            for &variable in &variables {
                if let Some(fragment) = self.declared_fragment_of(variable) {
                    let name = self.ast[variable].name;
                    self.check_for_incomplete_induced_accessors(name, fragment);
                }
            }
        }

        if !is_static
            && self.variable_list_is_const(fields)
            && let Some(keyword) = self.ast[fields].keyword
        {
            self.report_at_token(diag::const_instance_field(), keyword);
        }

        self.check_for_extension_declares_instance_field(node);
        self.check_for_extension_type_declares_instance_field(node);
        self.check_for_wrong_type_parameter_variance_in_field(node);
        self.check_for_late_final_field_with_const_constructor(node);
        self.check_for_non_final_field_in_enum(Some(node), None);

        // Dart `node.visitChildrenWithHooks(this, visitFields:)`.
        let fields_context = if is_static {
            ThisContext::StaticFieldDeclaration
        } else if self.ast[fields].late_keyword.is_some() {
            ThisContext::LateInstanceFieldDeclaration
        } else {
            ThisContext::InstanceFieldDeclaration
        };
        let ast = self.ast;
        for child in ast.children(node) {
            if child == fields.raw() {
                self.with_this_context(fields_context, |this| this.accept(child));
            } else {
                self.accept(child);
            }
        }
    }

    /// Dart `visitFieldFormalParameter`.
    pub(super) fn visit_field_formal_parameter(&mut self, node: Id<FieldFormalParameter>) {
        let as_formal: Id<FormalParameter> = Id::from_raw(node.raw());
        self.check_for_default_value_assignable_at_type(as_formal);
        self.check_for_valid_field(node);
        self.check_private_optional_parameter(as_formal);
        self.check_for_field_initializing_formal_redirecting_constructor(node);
        self.check_for_type_annotation_deferred_class(self.ast[node].type_);

        if let Some(element) = self.declared_element_of(node)
            && element.tag() == Tag::FieldFormalParameter
            && let Some(field) = self.field_formal_parameter_field(element)
        {
            let name = self.ast[node].name;
            self.check_for_abstract_or_external_field_constructor_initializer(
                Entity::Token(name),
                field,
            );
        }

        self.visit_children(node);
    }

    /// Dart `visitInstanceCreationExpression`.
    pub(super) fn visit_instance_creation_expression(
        &mut self,
        node: Id<InstanceCreationExpression>,
    ) {
        let constructor_name = self.ast[node].constructor_name;
        let named_type = self.ast[constructor_name].type_;
        if !self.is_in_unresolved_annotation(node.raw())
            && let Some(ty) = self.tables.annotation_type.get(named_type).copied()
            && let TypeKind::Interface { element, .. } = *self.ctx.ty(ty)
        {
            let constructor_element = self.element(constructor_name);
            self.check_for_const_or_new_with_abstract_class(node, named_type, element);
            self.check_for_invalid_generative_constructor_reference(
                constructor_name.raw(),
                constructor_element,
            );
            self.check_for_const_or_new_with_mixin(named_type, element);
            required_parameters_verifier::visit_instance_creation_expression(self, node);
            const_argument_verifier::visit_instance_creation_expression(self, node);
            use_result_verifier::check_instance_creation_expression(self, node);
            if crate::ast_ext::instance_creation_is_const(self.ast, node) {
                let keyword = self.ast[node].keyword;
                self.check_for_const_with_non_const(node.raw(), constructor_element, keyword);
                self.check_for_const_with_undefined_constructor(constructor_name, named_type);
                self.check_for_const_deferred_class(constructor_name, named_type);
            } else {
                self.check_for_new_with_undefined_constructor(constructor_name, named_type);
            }
        }
        self.visit_children(node);
    }

    /// Dart `visitPrimaryConstructorBody`.
    pub(super) fn visit_primary_constructor_body(&mut self, node: Id<PrimaryConstructorBody>) {
        // Not in Dart: the resolver does not resolve primary constructor
        // bodies yet (`ResolverVisitor::visit_primary_constructor_body` is a
        // stub), and the checks of unresolved nodes report false positives
        // (`initializer_for_non_existent_field`,
        // `const_with_undefined_constructor`, ...). Skip the body until the
        // resolver resolves it.
        if !self.subtree_is_resolved(node.raw()) {
            return;
        }
        let Some(declaration) =
            crate::element_binding_visitor::primary_constructor_body_declaration(self.ast, node)
        else {
            return;
        };
        let Some(fragment) = self.declared_fragment_of(declaration) else {
            return;
        };
        let Some(element) = self.fragment_element(fragment) else {
            return;
        };
        let context = EnclosingExecutableContext::new(
            &self.ctx,
            Some(element),
            self.fragment_has(fragment, FragmentFlags::EXECUTABLE_FRAGMENT_IS_ASYNCHRONOUS),
            self.fragment_has(fragment, FragmentFlags::EXECUTABLE_FRAGMENT_IS_GENERATOR),
        );
        self.with_enclosing_executable(context, |this| {
            this.check_for_conflicting_primary_constructor_initializers(node);
            let n = &this.ast[node];
            let (initializers, body) = (n.initializers, n.body);
            this.visit_children_with_hooks(
                node.raw(),
                initializers,
                body,
                ThisContext::GenerativeConstructorBody,
            );
        });
    }

    /// Dart `visitPrimaryConstructorDeclaration`.
    pub(super) fn visit_primary_constructor_declaration(
        &mut self,
        node: Id<PrimaryConstructorDeclaration>,
    ) {
        let Some(fragment) = self.declared_fragment_of(node) else {
            self.visit_children(node);
            return;
        };
        let Some(element) = self.fragment_element(fragment) else {
            self.visit_children(node);
            return;
        };
        let context = EnclosingExecutableContext::new(
            &self.ctx,
            Some(element),
            self.fragment_has(fragment, FragmentFlags::EXECUTABLE_FRAGMENT_IS_ASYNCHRONOUS),
            self.fragment_has(fragment, FragmentFlags::EXECUTABLE_FRAGMENT_IS_GENERATOR),
        );
        self.with_enclosing_executable(context, |this| {
            this.visit_children(node);
            let body = primary_constructor_declaration_body(this.ast, node);
            let error_range = this.primary_constructor_error_range(node);
            let initializers = body.map(|b| this.ast[b].initializers);
            let const_keyword = this.ast[node].const_keyword;

            if !this.check_for_const_constructor_with_non_const_super(
                None,
                initializers,
                element,
                error_range,
            ) {
                this.check_for_const_constructor_with_non_final_field(element, error_range);
                if let Some(body) = body {
                    let function_body = this.ast[body].body;
                    this.validate_constructor_body_allowed(
                        element,
                        const_keyword,
                        None,
                        false,
                        function_body,
                        true,
                    );
                }
            }

            let error_range = match body {
                Some(b) => this.token_range(this.ast[b].this_keyword),
                None => error_range,
            };
            let formal_parameters = this.ast[node].formal_parameters;
            this.check_for_undefined_constructor_in_initializer_implicit(
                formal_parameters,
                initializers,
                error_range,
            );
        });
        self.check_for_non_final_field_in_enum(None, Some(node));
    }

    /// Dart `visitRedirectingConstructorInvocation`.
    pub(super) fn visit_redirecting_constructor_invocation(
        &mut self,
        node: Id<RedirectingConstructorInvocation>,
    ) {
        required_parameters_verifier::visit_redirecting_constructor_invocation(self, node);
        const_argument_verifier::visit_redirecting_constructor_invocation(self, node);
        self.visit_children(node);
    }

    /// Dart `visitSuperConstructorInvocation`.
    pub(super) fn visit_super_constructor_invocation(
        &mut self,
        node: Id<SuperConstructorInvocation>,
    ) {
        let enclosing_constructor = self
            .enclosing_executable
            .element
            .filter(|e| e.tag() == Tag::Constructor);
        required_parameters_verifier::visit_super_constructor_invocation(
            self,
            node,
            enclosing_constructor.map(ElemRef::from),
        );
        const_argument_verifier::visit_super_constructor_invocation(self, node);
        self.check_for_extension_type_constructor_with_super_invocation(node);
        self.visit_children(node);
    }

    /// Dart `visitSuperFormalParameter`.
    pub(super) fn visit_super_formal_parameter(&mut self, node: Id<SuperFormalParameter>) {
        let as_formal: Id<FormalParameter> = Id::from_raw(node.raw());
        self.check_for_default_value_assignable_at_type(as_formal);
        self.check_private_optional_parameter(as_formal);
        self.visit_children(node);

        let super_keyword = self.ast[node].super_keyword;
        let constructor = self.ast.parent(node).and_then(|list| self.ast.parent(list));

        if self
            .enclosing_class
            .is_some_and(|c| c.raw().tag() == Tag::ExtensionType)
        {
            if constructor.is_some_and(|c| self.ast.is::<PrimaryConstructorDeclaration>(c)) {
                return;
            }
            self.report_at_token(
                diag::extension_type_constructor_with_super_formal_parameter(),
                super_keyword,
            );
            return;
        }

        let constructor_declaration = constructor.and_then(|c| {
            self.ast
                .cast::<ConstructorDeclaration>(c)
                .filter(|&c| self.ast.is_non_redirecting_generative(c))
                .map(|c| c.raw())
        });
        let primary = constructor
            .and_then(|c| self.ast.cast::<PrimaryConstructorDeclaration>(c))
            .map(|p| p.raw());
        if let Some(c) = constructor_declaration.or(primary) {
            let Some(constructor_element) = self.declared_element_of(c) else {
                return;
            };
            if self.super_constructor(constructor_element).is_none() {
                return;
            }
        } else {
            self.report_at_token(
                diag::invalid_super_formal_parameter_location(),
                super_keyword,
            );
            return;
        }

        let Some(element) = self.declared_element_of(node) else {
            return;
        };
        if element.tag() != Tag::SuperFormalParameter {
            return;
        }

        let name = self.ast[node].name;
        let is_named = self.ast[node].kind.is_named();
        let Some(super_parameter) = self.super_constructor_parameter(element) else {
            let d = if is_named {
                diag::super_formal_parameter_without_associated_named()
            } else {
                diag::super_formal_parameter_without_associated_positional()
            };
            self.report_at_token(d, name);
            return;
        };

        let ctx = self.ctx;
        let element_type = variable_type(&ctx, element);
        let super_parameter_type = member::type_(&ctx, super_parameter);
        if !self
            .type_system
            .is_subtype_of(element_type, super_parameter_type)
        {
            self.report_at_token(
                diag::super_formal_parameter_type_is_not_subtype_of_associated(
                    type_arg(&ctx, element_type),
                    type_arg(&ctx, super_parameter_type),
                ),
                name,
            );
        }
    }

    /// Dart `visitTopLevelVariableDeclaration`.
    pub(super) fn visit_top_level_variable_declaration(
        &mut self,
        node: Id<TopLevelVariableDeclaration>,
    ) {
        let n = &self.ast[node];
        let (augment_keyword, abstract_keyword, external_keyword, variable_list) = (
            n.augment_keyword,
            n.abstract_keyword,
            n.external_keyword,
            n.variables,
        );
        let variables: Vec<Id<VariableDeclaration>> =
            self.ast.list(self.ast[variable_list].variables).to_vec();
        let type_node = self.ast[variable_list].type_;

        for &variable in &variables {
            if let Some(declared_fragment) = self.declared_fragment_of(variable) {
                self.check_variable_augmentation(
                    augment_keyword,
                    variable,
                    declared_fragment,
                    type_node,
                );
            }
        }

        if self.variable_list_is_const(variable_list) {
            for &variable in &variables {
                if self.ast[variable].initializer.is_none() {
                    let name = self.ast[variable].name;
                    let d = diag::const_not_initialized(self.ast.tokens.lexeme(name));
                    self.report_at_token(d, name);
                }
            }
        } else if abstract_keyword.is_none()
            && external_keyword.is_none()
            && self.ast[variable_list].late_keyword.is_none()
        {
            let is_final = self.variable_list_is_final(variable_list);
            for &variable in &variables {
                if self.ast[variable].initializer.is_some() {
                    continue;
                }
                let name = self.ast[variable].name;
                if is_final {
                    let d = diag::final_not_initialized(self.ast.tokens.lexeme(name));
                    self.report_at_token(d, name);
                } else if let Some(element) = self.declared_element_of(variable) {
                    let ty = variable_type(&self.ctx, element);
                    if self.type_system.is_potentially_non_nullable(ty) {
                        let d = diag::not_initialized_non_nullable_variable(
                            self.ast.tokens.lexeme(name),
                        );
                        self.report_at_token(d, name);
                    }
                }
            }
        }

        for &variable in &variables {
            let Some(fragment) = self.declared_fragment_of(variable) else {
                continue;
            };
            let name = self.ast[variable].name;
            if abstract_keyword.is_some() {
                self.check_for_incomplete_induced_accessors(name, fragment);
            }
            self.check_for_main_function1(name, fragment);
        }

        self.visit_children(node);
    }

    /// Dart `visitVariableDeclaration`.
    pub(super) fn visit_variable_declaration(&mut self, node: Id<VariableDeclaration>) {
        let name_token = self.ast[node].name;
        let initializer_node = self.ast[node].initializer;
        // do checks
        self.check_for_abstract_or_external_variable_initializer(node);
        // visit initializer
        let name = self.ast.tokens.lexeme(name_token).to_string();
        self.names_for_reference_to_declared_variable_in_initializer
            .insert(name.clone());
        if let Some(initializer_node) = initializer_node {
            self.accept(initializer_node);
        }
        self.names_for_reference_to_declared_variable_in_initializer
            .shift_remove(&name);
        // declare the variable
        let grandparent = self.ast.parent(node).and_then(|p| self.ast.parent(p));
        let is_top_level_or_field = grandparent.is_some_and(|g| {
            self.ast.is::<TopLevelVariableDeclaration>(g) || self.ast.is::<FieldDeclaration>(g)
        });
        if !is_top_level_or_field
            && let Some(element) = self.declared_element_of(node)
            && let Some(hidden) = self.hidden_elements.as_mut()
        {
            // There is no hidden elements if we are outside of a function
            // body, which will happen for variables declared in control flow
            // elements.
            hidden.declare(element);
        }
    }

    /// Dart `visitVariableDeclarationList`.
    pub(super) fn visit_variable_declaration_list(&mut self, node: Id<VariableDeclarationList>) {
        self.check_for_type_annotation_deferred_class(self.ast[node].type_);
        self.visit_children(node);
    }

    // ------------------------------------------------------------ helpers

    /// Dart `node.visitChildrenWithHooks(this, visitInitializers:,
    /// visitBody:)` of a constructor declaration or a primary constructor
    /// body.
    fn visit_children_with_hooks(
        &mut self,
        node: NodeId,
        initializers: NodeList<ConstructorInitializer>,
        body: Id<FunctionBody>,
        body_context: ThisContext,
    ) {
        let initializer_ids: IndexSet<NodeId> =
            self.ast.list_raw(initializers).iter().copied().collect();
        let ast = self.ast;
        for child in ast.children(node) {
            if initializer_ids.contains(&child) {
                self.with_this_context(ThisContext::ConstructorInitializers, |this| {
                    this.accept(child);
                });
            } else if child == body.raw() {
                self.with_this_context(body_context, |this| this.accept(child));
            } else {
                self.accept(child);
            }
        }
    }

    /// The augmentation checks of each variable of
    /// `visitFieldDeclaration` and `visitTopLevelVariableDeclaration`.
    fn check_variable_augmentation(
        &mut self,
        augment_keyword: Option<TokenId>,
        variable: Id<VariableDeclaration>,
        declared_fragment: FragmentId,
        type_node: Option<Id<TypeAnnotation>>,
    ) {
        let name = self.ast[variable].name;
        let has_const_variable_augmentation =
            self.check_for_const_variable_augmentation(name, declared_fragment);
        if augment_keyword.is_some() && !has_const_variable_augmentation {
            self.check_augmentation_without_declaration(Some(name), declared_fragment);
            self.check_augmentation_without_declaration_for_induced_accessors(
                name,
                declared_fragment,
            );
            self.check_for_augmentation_induced_accessors_already_complete(name, declared_fragment);
            if let Some(induced_getter) = self.induced_getter(declared_fragment) {
                self.check_for_augmentation_return_type_mismatch(
                    induced_getter,
                    type_node,
                    Entity::Token(name),
                );
            }
        }
    }

    /// Dart `_checkAugmentationWithoutDeclarationForInducedAccessors`.
    fn check_augmentation_without_declaration_for_induced_accessors(
        &mut self,
        variable_name: TokenId,
        fragment: FragmentId,
    ) {
        // Already reported by `_checkAugmentationWithoutDeclaration`.
        if self.previous_fragment(fragment).is_none() {
            return;
        }
        let name = self.ast.tokens.lexeme(variable_name).to_string();
        if let Some(induced_getter) = self.induced_getter(fragment)
            && self.previous_fragment(induced_getter).is_none()
        {
            self.report_at_token(
                diag::augmentation_without_getter_declaration(&name),
                variable_name,
            );
        }
        if let Some(induced_setter) = self.induced_setter(fragment)
            && self.previous_fragment(induced_setter).is_none()
        {
            self.report_at_token(
                diag::augmentation_without_setter_declaration(&name),
                variable_name,
            );
        }
    }

    /// Dart `_checkForAbstractOrExternalFieldConstructorInitializer`.
    fn check_for_abstract_or_external_field_constructor_initializer(
        &mut self,
        identifier: Entity,
        field_element: ElementId,
    ) {
        let flags = first_fragment_flags(&self.ctx, field_element);
        if flags.contains(FragmentFlags::VARIABLE_FRAGMENT_IS_ABSTRACT) {
            self.report_at_entity(diag::abstract_field_constructor_initializer(), identifier);
        }
        if flags.contains(FragmentFlags::VARIABLE_FRAGMENT_IS_EXTERNAL) {
            self.report_at_entity(diag::external_field_constructor_initializer(), identifier);
        }
    }

    /// Dart `_checkForAbstractOrExternalVariableInitializer`.
    fn check_for_abstract_or_external_variable_initializer(
        &mut self,
        node: Id<VariableDeclaration>,
    ) {
        if self.ast[node].initializer.is_none() {
            return;
        }
        let Some(declared_element) = self.declared_element_of(node) else {
            return;
        };
        let name = self.ast[node].name;
        let flags = first_fragment_flags(&self.ctx, declared_element);
        match declared_element.tag() {
            Tag::Field => {
                if flags.contains(FragmentFlags::VARIABLE_FRAGMENT_IS_ABSTRACT) {
                    self.report_at_token(diag::abstract_field_initializer(), name);
                }
                if flags.contains(FragmentFlags::VARIABLE_FRAGMENT_IS_EXTERNAL) {
                    self.report_at_token(diag::external_field_initializer(), name);
                }
            }
            Tag::TopLevelVariable
                if flags.contains(FragmentFlags::VARIABLE_FRAGMENT_IS_EXTERNAL) =>
            {
                self.report_at_token(diag::external_variable_initializer(), name);
            }
            _ => {}
        }
    }

    /// Dart `_checkForAllRedirectConstructorErrorCodes`.
    fn check_for_all_redirect_constructor_error_codes(
        &mut self,
        declaration: Id<ConstructorDeclaration>,
    ) {
        // Prepare redirected constructor node
        let Some(redirected_constructor) = self.ast[declaration].redirected_constructor else {
            return;
        };
        let ctx = self.ctx;

        // Prepare redirected constructor type
        let Some(redirected_element) = self.element(redirected_constructor) else {
            // If the element is null, we check for the
            // REDIRECT_TO_MISSING_CONSTRUCTOR case
            let constructor_named_type = self.ast[redirected_constructor].type_;
            let Some(redirected_type) = self
                .tables
                .annotation_type
                .get(constructor_named_type)
                .copied()
            else {
                return;
            };
            if !matches!(
                ctx.ty(redirected_type),
                TypeKind::Dynamic | TypeKind::Invalid
            ) {
                // Prepare the constructor name
                let mut constructor_str_name = self.ast.qualified_name(constructor_named_type);
                if let Some(name) = self.ast[redirected_constructor].name {
                    constructor_str_name.push('.');
                    constructor_str_name.push_str(self.identifier_lexeme(name));
                }
                self.report_at(
                    diag::redirect_to_missing_constructor(
                        &constructor_str_name,
                        type_arg(&ctx, redirected_type),
                    ),
                    redirected_constructor,
                );
            }
            return;
        };
        let redirected_type = member::type_(&ctx, redirected_element);
        let Some(redirected_return_type) = function_return_type(&ctx, redirected_type) else {
            return;
        };

        // Report specific problem when return type is incompatible
        let Some(element) = self.declared_element_of(declaration) else {
            return;
        };
        let constructor_type = member::type_(&ctx, ElemRef::Base(element));
        let Some(constructor_return_type) = function_return_type(&ctx, constructor_type) else {
            return;
        };
        if !self.type_system.is_assignable_to(
            redirected_return_type,
            constructor_return_type,
            self.strict_casts(),
        ) {
            self.report_at(
                diag::redirect_to_invalid_return_type(
                    type_arg(&ctx, redirected_return_type),
                    type_arg(&ctx, constructor_return_type),
                ),
                redirected_constructor,
            );
        } else if !self
            .type_system
            .is_subtype_of(redirected_type, constructor_type)
        {
            // Check parameters.
            self.report_at(
                diag::redirect_to_invalid_function_type(
                    type_arg(&ctx, redirected_type),
                    type_arg(&ctx, constructor_type),
                ),
                redirected_constructor,
            );
        }
    }

    /// Dart `_checkForAugmentationInducedAccessorsAlreadyComplete`.
    fn check_for_augmentation_induced_accessors_already_complete(
        &mut self,
        error_token: TokenId,
        fragment: FragmentId,
    ) {
        if let Some(induced_getter) = self.induced_getter(fragment)
            && self.fragment_has(induced_getter, FragmentFlags::FRAGMENT_IS_COMPLETE)
            && self
                .nearest_preceding_complete_fragment(induced_getter)
                .is_some()
        {
            self.report_at_token(
                diag::augmentation_induced_getter_already_complete(),
                error_token,
            );
        }
        if let Some(induced_setter) = self.induced_setter(fragment)
            && self.fragment_has(induced_setter, FragmentFlags::FRAGMENT_IS_COMPLETE)
            && self
                .nearest_preceding_complete_fragment(induced_setter)
                .is_some()
        {
            self.report_at_token(
                diag::augmentation_induced_setter_already_complete(),
                error_token,
            );
        }
    }

    /// Dart `_checkForConflictingInitializerErrorCodes`.
    fn check_for_conflicting_initializer_error_codes(
        &mut self,
        declaration: Id<ConstructorDeclaration>,
    ) {
        let Some(enclosing_class) = self.enclosing_class else {
            return;
        };
        let is_enum = enclosing_class.raw().tag() == Tag::Enum;
        let initializers: Vec<NodeId> = self
            .ast
            .list_raw(self.ast[declaration].initializers)
            .to_vec();
        let factory_keyword = self.ast[declaration].factory_keyword;
        // Count and check each redirecting initializer.
        let mut redirecting_initializer_count = 0;
        let mut super_initializer_count = 0;
        let mut super_initializer: Option<Id<SuperConstructorInvocation>> = None;
        for &initializer in &initializers {
            if let Some(invocation) = self
                .ast
                .cast::<RedirectingConstructorInvocation>(initializer)
            {
                if redirecting_initializer_count > 0 {
                    self.report_at(
                        diag::multiple_redirecting_constructor_invocations(),
                        invocation,
                    );
                }
                let redirecting_element = self.element(invocation);
                let constructor_name = self.ast[invocation].constructor_name;
                if factory_keyword.is_none() {
                    match redirecting_element {
                        None => {
                            let enclosing_named_type = self.element_name(enclosing_class.raw());
                            let mut constructor_str_name = enclosing_named_type.clone();
                            if let Some(name) = constructor_name {
                                constructor_str_name.push('.');
                                constructor_str_name.push_str(self.identifier_lexeme(name));
                            }
                            self.report_at(
                                diag::redirect_generative_to_missing_constructor(
                                    &constructor_str_name,
                                    &enclosing_named_type,
                                ),
                                invocation,
                            );
                        }
                        Some(redirecting) => {
                            let base = member::base_element(&self.ctx, redirecting);
                            if self.constructor_is_factory(base) {
                                self.report_at(
                                    diag::redirect_generative_to_non_generative_constructor(),
                                    invocation,
                                );
                            }
                        }
                    }
                }
                // [declaration] is a redirecting constructor via a
                // redirecting initializer.
                if let Some(element) = self.declared_element_of(declaration) {
                    let error_entity = match constructor_name {
                        Some(n) => Entity::Node(n.raw()),
                        None => Entity::Token(self.ast[invocation].this_keyword),
                    };
                    self.check_for_redirect_to_non_const_constructor(
                        element,
                        redirecting_element,
                        error_entity,
                    );
                }
                redirecting_initializer_count += 1;
            } else if let Some(initializer) =
                self.ast.cast::<SuperConstructorInvocation>(initializer)
            {
                if is_enum {
                    self.report_at_token(
                        diag::super_in_enum_constructor(),
                        self.ast[initializer].super_keyword,
                    );
                } else if super_initializer_count == 1 {
                    // Only report the second (first illegal) superinitializer.
                    self.report_at(diag::multiple_super_initializers(), initializer);
                }
                super_initializer = Some(initializer);
                super_initializer_count += 1;
            }
        }
        // Check for initializers which are illegal when alongside a
        // redirecting initializer.
        if redirecting_initializer_count > 0 {
            for &initializer in &initializers {
                if self.ast.is::<SuperConstructorInvocation>(initializer) && !is_enum {
                    self.report_at(diag::super_in_redirecting_constructor(), initializer);
                }
                if self.ast.is::<ConstructorFieldInitializer>(initializer) {
                    self.report_at(
                        diag::field_initializer_redirecting_constructor(),
                        initializer,
                    );
                }
                if self.ast.is::<AssertInitializer>(initializer) {
                    self.report_at(diag::assert_in_redirecting_constructor(), initializer);
                }
            }
        }
        if !is_enum
            && redirecting_initializer_count == 0
            && super_initializer_count == 1
            && let Some(super_initializer) = super_initializer
            && initializers.last() != Some(&super_initializer.raw())
            && self
                .ctx
                .interface(enclosing_class)
                .supertype
                .get()
                .is_some()
        {
            self.report_at_token(
                diag::super_invocation_not_last(),
                self.ast[super_initializer].super_keyword,
            );
        }
    }

    /// Dart `_checkForConflictingPrimaryConstructorInitializers`: checks that
    /// the primary constructor [body] has a valid combination of redirecting
    /// constructor invocation(s) and super constructor invocation(s).
    fn check_for_conflicting_primary_constructor_initializers(
        &mut self,
        body: Id<PrimaryConstructorBody>,
    ) {
        let initializers: Vec<NodeId> = self.ast.list_raw(self.ast[body].initializers).to_vec();
        let redirecting: Vec<Id<RedirectingConstructorInvocation>> = initializers
            .iter()
            .filter_map(|&i| self.ast.cast::<RedirectingConstructorInvocation>(i))
            .collect();
        if !redirecting.is_empty() {
            for invocation in redirecting {
                self.report_at_token(
                    diag::primary_constructor_cannot_redirect(),
                    self.ast[invocation].this_keyword,
                );
            }
            return;
        }

        let super_invocations: Vec<Id<SuperConstructorInvocation>> = initializers
            .iter()
            .filter_map(|&i| self.ast.cast::<SuperConstructorInvocation>(i))
            .collect();
        match self.enclosing_class.map(|c| c.raw().tag()) {
            Some(Tag::Class) => {
                if let [_, second, ..] = super_invocations[..] {
                    self.report_at_token(
                        diag::multiple_super_initializers(),
                        self.ast[second].super_keyword,
                    );
                    return;
                }
            }
            Some(Tag::Enum) => {
                if let [first, ..] = super_invocations[..] {
                    self.report_at_token(
                        diag::super_in_enum_constructor(),
                        self.ast[first].super_keyword,
                    );
                    return;
                }
            }
            _ => {}
        }

        if let Some(&last_super) = super_invocations.last()
            && initializers.last() != Some(&last_super.raw())
        {
            self.report_at_token(
                diag::super_invocation_not_last(),
                self.ast[last_super].super_keyword,
            );
        }
    }

    /// Dart `_checkForConstConstructorWithNonConstSuper`: whether an error
    /// is reported here, and the caller should stop checking the constructor
    /// for constant-related errors.
    fn check_for_const_constructor_with_non_const_super(
        &mut self,
        factory_keyword: Option<TokenId>,
        initializers: Option<NodeList<ConstructorInitializer>>,
        element: ElementId,
        implicit_error_range: SourceRange,
    ) -> bool {
        let ctx = self.ctx;
        let Some(enclosing_class) = self.enclosing_interface_of(element) else {
            return false;
        };
        if !self.constructor_is_const(element) {
            return false;
        }

        // OK, const factory, checked elsewhere
        if factory_keyword.is_some() {
            return false;
        }

        // check for mixins
        let mut instance_fields: Vec<ElementId> = Vec::new();
        let mixins = ctx
            .interface(enclosing_class)
            .mixins
            .get()
            .unwrap_or_default();
        for &mixin in ctx.list(mixins) {
            let TypeKind::Interface {
                element: mixin_element,
                ..
            } = *ctx.ty(mixin)
            else {
                continue;
            };
            for &field in &ctx.interface(mixin_element).fields {
                let flags = first_fragment_flags(&ctx, field.raw());
                if flags.contains(FragmentFlags::VARIABLE_FRAGMENT_IS_STATIC) {
                    continue;
                }
                if flags.contains(FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_GETTER_SETTER)
                {
                    continue;
                }
                // From the abstract and external fields specification:
                // > An abstract instance variable declaration D is treated as
                // > an abstract getter declaration and possibly an abstract
                // > setter declaration. The setter is included if and only if
                // > D is non-final.
                if flags.contains(FragmentFlags::VARIABLE_FRAGMENT_IS_ABSTRACT)
                    && flags.contains(FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL)
                {
                    continue;
                }
                instance_fields.push(field.raw());
            }
        }

        let field_name = |this: &Self, field: ElementId| -> String {
            let enclosing = ctx
                .element_data(field)
                .and_then(|d| d.enclosing)
                .map(|e| this.element_name(e))
                .unwrap_or_default();
            format!("'{enclosing}.{}'", this.element_name(field))
        };

        if instance_fields.len() == 1 {
            let name = field_name(self, instance_fields[0]);
            self.report_at_range(
                diag::const_constructor_with_mixin_with_field(&name),
                implicit_error_range,
            );
            return true;
        } else if instance_fields.len() > 1 {
            let names: Vec<String> = instance_fields
                .iter()
                .map(|&f| field_name(self, f))
                .collect();
            self.report_at_range(
                diag::const_constructor_with_mixin_with_fields(&names.join(", ")),
                implicit_error_range,
            );
            return true;
        }

        // Enum(s) always call a const super-constructor.
        if enclosing_class.raw().tag() == Tag::Enum {
            return false;
        }

        // Redirecting constructors are checked to be const elsewhere.
        if self.redirected_constructor(element).is_some() {
            return false;
        }

        let Some(invoked_super) = self.super_constructor(element) else {
            return false;
        };
        let invoked_super_base = member::base_element(&ctx, invoked_super);
        if self.constructor_is_const(invoked_super_base) {
            return false;
        }

        // Often there is an explicit `super()` invocation, report on it.
        let super_invocation = initializers.and_then(|list| {
            self.ast
                .list_raw(list)
                .iter()
                .copied()
                .find(|&i| self.ast.is::<SuperConstructorInvocation>(i))
        });
        let error_range = match super_invocation {
            Some(i) => self.node_range(i),
            None => implicit_error_range,
        };
        let superclass_name = ctx
            .element_data(invoked_super_base)
            .and_then(|d| d.enclosing)
            .map(|e| self.element_name(e))
            .unwrap_or_default();
        self.report_at_range(
            diag::const_constructor_with_non_const_super(&superclass_name),
            error_range,
        );
        true
    }

    /// Dart `_checkForConstConstructorWithNonFinalField`.
    fn check_for_const_constructor_with_non_final_field(
        &mut self,
        constructor_element: ElementId,
        error_range: SourceRange,
    ) {
        if !self.constructor_is_const(constructor_element) {
            return;
        }
        if self.constructor_is_factory(constructor_element) {
            return;
        }
        // check if there is non-final field
        let Some(class_element) = self.enclosing_interface_of(constructor_element) else {
            return;
        };
        if class_element.raw().tag() != Tag::Class
            || !self.ctx.interface(class_element).has_non_final_field.get()
        {
            return;
        }
        self.report_at_range(diag::const_constructor_with_non_final_field(), error_range);
    }

    /// Dart `_checkForConstDeferredClass`.
    fn check_for_const_deferred_class(
        &mut self,
        constructor_name: Id<ConstructorName>,
        named_type: Id<NamedType>,
    ) {
        if self.named_type_is_deferred(named_type) {
            self.report_at(diag::const_deferred_class(), constructor_name);
        }
    }

    /// Dart `_checkForConstOrNewWithAbstractClass`.
    fn check_for_const_or_new_with_abstract_class(
        &mut self,
        expression: Id<InstanceCreationExpression>,
        named_type: Id<NamedType>,
        element: EId<InterfaceElement>,
    ) {
        if element.raw().tag() == Tag::Class && self.class_is_abstract(element.raw()) {
            let constructor_name = self.ast[expression].constructor_name;
            if let Some(constructor_element) = self.element(constructor_name) {
                let base = member::base_element(&self.ctx, constructor_element);
                if !self.constructor_is_factory(base) {
                    self.report_at(diag::instantiate_abstract_class(), named_type);
                }
            }
        }
    }

    /// Dart `_checkForConstOrNewWithMixin`.
    fn check_for_const_or_new_with_mixin(
        &mut self,
        named_type: Id<NamedType>,
        element: EId<InterfaceElement>,
    ) {
        if element.raw().tag() == Tag::Mixin {
            self.report_at(diag::mixin_instantiate(), named_type);
        }
    }

    /// Dart `_checkForConstructorAugmentationModifierMismatch`.
    fn check_for_constructor_augmentation_modifier_mismatch(
        &mut self,
        node: Id<ConstructorDeclaration>,
        declared_fragment: FragmentId,
    ) {
        let Some(augment_keyword) = self.ast[node].augment_keyword else {
            return;
        };
        let Some(element) = self.fragment_element(declared_fragment) else {
            return;
        };
        let Some(first_fragment) = self.ctx.element_data(element).map(|d| d.first_fragment) else {
            return;
        };
        if first_fragment == declared_fragment {
            return;
        }
        let (const_keyword, factory_keyword) =
            (self.ast[node].const_keyword, self.ast[node].factory_keyword);
        let flag = FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_CONST;
        self.check_for_augmentation_modifier_mismatch(
            augment_keyword,
            self.fragment_has(declared_fragment, flag),
            self.fragment_has(first_fragment, flag),
            const_keyword,
            "const",
        );
        let flag = FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY;
        self.check_for_augmentation_modifier_mismatch(
            augment_keyword,
            self.fragment_has(declared_fragment, flag),
            self.fragment_has(first_fragment, flag),
            factory_keyword,
            "factory",
        );
    }

    /// Dart `_checkForConstWithNonConst`.
    fn check_for_const_with_non_const(
        &mut self,
        expression: NodeId,
        constructor_element: Option<ElemRef>,
        keyword: Option<TokenId>,
    ) {
        let Some(constructor_element) = constructor_element else {
            return;
        };
        let base = member::base_element(&self.ctx, constructor_element);
        if base.tag() != Tag::Constructor || self.constructor_is_const(base) {
            return;
        }
        match keyword {
            Some(keyword) => self.report_at_token(diag::const_with_non_const(), keyword),
            None => self.report_at(diag::const_with_non_const(), expression),
        }
    }

    /// Dart `_checkForConstWithUndefinedConstructor`.
    fn check_for_const_with_undefined_constructor(
        &mut self,
        constructor_name: Id<ConstructorName>,
        named_type: Id<NamedType>,
    ) {
        // OK if resolved
        if self.element(constructor_name).is_some() {
            return;
        }
        let class_name = self.ast.qualified_name(named_type);
        // report as named or default constructor absence
        match self.ast[constructor_name].name {
            Some(name) => {
                let d = diag::const_with_undefined_constructor(
                    &class_name,
                    self.identifier_lexeme(name),
                );
                self.report_at(d, name);
            }
            None => {
                self.report_at(
                    diag::const_with_undefined_constructor_default(&class_name),
                    constructor_name,
                );
            }
        }
    }

    /// Dart `_checkForExtensionDeclaresInstanceField`.
    fn check_for_extension_declares_instance_field(&mut self, node: Id<FieldDeclaration>) {
        let in_extension = self
            .ast
            .parent(node)
            .and_then(|p| self.ast.parent(p))
            .is_some_and(|g| self.ast.is::<ExtensionDeclaration>(g));
        if !in_extension {
            return;
        }
        let n = &self.ast[node];
        if n.static_keyword.is_some() || n.external_keyword.is_some() {
            return;
        }
        for &field in self.ast.list(self.ast[n.fields].variables) {
            self.report_at_token(
                diag::extension_declares_instance_field(),
                self.ast[field].name,
            );
        }
    }

    /// Dart `_checkForExtensionTypeConstructorWithSuperInvocation`.
    fn check_for_extension_type_constructor_with_super_invocation(
        &mut self,
        node: Id<SuperConstructorInvocation>,
    ) {
        if self
            .enclosing_class
            .is_some_and(|c| c.raw().tag() == Tag::ExtensionType)
        {
            self.report_at_token(
                diag::extension_type_constructor_with_super_invocation(),
                self.ast[node].super_keyword,
            );
        }
    }

    /// Dart `_checkForExtensionTypeDeclaresInstanceField`.
    fn check_for_extension_type_declares_instance_field(&mut self, node: Id<FieldDeclaration>) {
        if !self
            .enclosing_class
            .is_some_and(|c| c.raw().tag() == Tag::ExtensionType)
        {
            return;
        }
        let n = &self.ast[node];
        if n.static_keyword.is_some() || n.external_keyword.is_some() {
            return;
        }
        for &field in self.ast.list(self.ast[n.fields].variables) {
            self.report_at_token(
                diag::extension_type_declares_instance_field(),
                self.ast[field].name,
            );
        }
    }

    /// Dart `_checkForFactoryBodyCompleteness`.
    fn check_for_factory_body_completeness(&mut self, node: Id<ConstructorDeclaration>) {
        if !self.unit_feature_enabled(ExperimentalFlag::Augmentations) {
            return;
        }
        // Report only on the introductory declaration.
        if self.ast[node].augment_keyword.is_some() {
            return;
        }
        if self.ast[node].factory_keyword.is_none() {
            return;
        }
        let Some(element) = self.declared_element_of(node) else {
            return;
        };
        let fragments = self.element_fragments(element);
        if fragments
            .iter()
            .any(|&f| self.fragment_has(f, FragmentFlags::FRAGMENT_IS_COMPLETE))
        {
            return;
        }
        let range = self.constructor_error_range(node);
        if fragments.len() == 1 {
            self.report_at_range(diag::factory_without_body(), range);
        } else {
            let name = self.element_name(element);
            self.report_at_range(diag::factory_not_complete_after_augmentations(&name), range);
        }
    }

    /// Dart `_checkForFieldInitializingFormalRedirectingConstructor`.
    fn check_for_field_initializing_formal_redirecting_constructor(
        &mut self,
        parameter: Id<FieldFormalParameter>,
    ) {
        // prepare the node that should be a ConstructorDeclaration
        let mut formal_parameter_list = self.ast.parent(parameter);
        if let Some(l) = formal_parameter_list
            && !self.ast.is::<FormalParameterList>(l)
        {
            formal_parameter_list = self.ast.parent(l);
        }
        let constructor = formal_parameter_list.and_then(|l| self.ast.parent(l));
        // now check whether the node is actually a ConstructorDeclaration
        if let Some(constructor) =
            constructor.and_then(|c| self.ast.cast::<ConstructorDeclaration>(c))
        {
            // constructor cannot be a factory
            if self.ast[constructor].factory_keyword.is_some() {
                self.report_at(diag::field_initializer_factory_constructor(), parameter);
                return;
            }
            // constructor cannot have a redirection
            let initializers = self.ast[constructor].initializers;
            if self
                .ast
                .list_raw(initializers)
                .iter()
                .any(|&i| self.ast.is::<RedirectingConstructorInvocation>(i))
            {
                self.report_at(diag::field_initializer_redirecting_constructor(), parameter);
            }
        } else if constructor.is_some_and(|c| self.ast.is::<PrimaryConstructorDeclaration>(c)) {
            // No additional checks.
        } else {
            self.report_at_token(
                diag::field_initializer_outside_constructor(),
                self.ast[parameter].this_keyword,
            );
        }
    }

    /// Dart `_checkForIncompleteInducedAccessors`.
    fn check_for_incomplete_induced_accessors(
        &mut self,
        name_token: TokenId,
        fragment: FragmentId,
    ) {
        if !self.unit_feature_enabled(ExperimentalFlag::Augmentations) {
            return;
        }
        if self.fragment_has(fragment, FragmentFlags::FRAGMENT_IS_AUGMENTATION) {
            return;
        }
        let getter = self.induced_getter(fragment);
        let setter = self.induced_setter(fragment);
        let fragments_of = |this: &Self, f: Option<FragmentId>| -> Vec<FragmentId> {
            f.and_then(|f| this.fragment_element(f))
                .map(|e| this.element_fragments(e))
                .unwrap_or_default()
        };
        let getter_fragments = fragments_of(self, getter);
        let setter_fragments = fragments_of(self, setter);
        let has_augmentations = getter_fragments.len() > 1 || setter_fragments.len() > 1;
        let name = self.ast.tokens.lexeme(name_token).to_string();
        let none_complete = |this: &Self, fragments: &[FragmentId]| {
            !fragments
                .iter()
                .any(|&f| this.fragment_has(f, FragmentFlags::FRAGMENT_IS_COMPLETE))
        };

        if getter.is_some() && none_complete(self, &getter_fragments) {
            let d = if has_augmentations {
                diag::induced_getter_not_complete_after_augmentations(&name)
            } else {
                diag::induced_getter_without_body(&name)
            };
            self.report_at_token(d, name_token);
        }
        if setter.is_some() && none_complete(self, &setter_fragments) {
            let d = if has_augmentations {
                diag::induced_setter_not_complete_after_augmentations(&name)
            } else {
                diag::induced_setter_without_body(&name)
            };
            self.report_at_token(d, name_token);
        }
    }

    /// Dart `_checkForInvalidField`.
    fn check_for_invalid_field(
        &mut self,
        initializer: Id<ConstructorFieldInitializer>,
        field_name: Id<SimpleIdentifier>,
        static_element: Option<ElemRef>,
    ) {
        let name = self.identifier_lexeme(field_name).to_string();
        let field = static_element
            .map(|e| member::base_element(&self.ctx, e))
            .filter(|e| e.tag() == Tag::Field);
        match field {
            Some(field) => {
                let flags = first_fragment_flags(&self.ctx, field);
                if flags.contains(FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_GETTER_SETTER)
                {
                    self.report_at(diag::initializer_for_non_existent_field(&name), initializer);
                } else if flags.contains(FragmentFlags::VARIABLE_FRAGMENT_IS_STATIC) {
                    self.report_at(diag::initializer_for_static_field(&name), initializer);
                }
            }
            None => {
                self.report_at(diag::initializer_for_non_existent_field(&name), initializer);
            }
        }
    }

    /// Dart `_checkForInvalidModifierOnBody`.
    fn check_for_invalid_modifier_on_body(&mut self, body: Id<FunctionBody>) {
        let keyword = if let Some(b) = self.ast.cast::<BlockFunctionBody>(body) {
            self.ast[b].keyword
        } else if let Some(b) = self.ast.cast::<ExpressionFunctionBody>(body) {
            self.ast[b].keyword
        } else {
            None
        };
        if let Some(keyword) = keyword {
            let d = diag::invalid_modifier_on_constructor(self.ast.tokens.lexeme(keyword));
            self.report_at_token(d, keyword);
        }
    }

    /// Dart `_checkForLateFinalFieldWithConstConstructor`.
    fn check_for_late_final_field_with_const_constructor(&mut self, node: Id<FieldDeclaration>) {
        if self.ast[node].static_keyword.is_some() {
            return;
        }
        let variable_list = self.ast[node].fields;
        if !self.variable_list_is_final(variable_list) {
            return;
        }
        let Some(late_keyword) = self.ast[variable_list].late_keyword else {
            return;
        };
        // The field is in an extension and should be handled elsewhere.
        let Some(enclosing_class) = self.enclosing_class else {
            return;
        };
        let has_generative_const_constructor = self
            .ctx
            .interface(enclosing_class)
            .constructors
            .iter()
            .any(|&c| self.constructor_is_const(c.raw()) && !self.constructor_is_factory(c.raw()));
        if !has_generative_const_constructor {
            return;
        }
        self.report_at_token(
            diag::late_final_field_with_const_constructor(),
            late_keyword,
        );
    }

    /// Dart `_checkForNewWithUndefinedConstructor`.
    fn check_for_new_with_undefined_constructor(
        &mut self,
        constructor_name: Id<ConstructorName>,
        named_type: Id<NamedType>,
    ) {
        // OK if resolved
        if self.element(constructor_name).is_some() {
            return;
        }
        if let Some(ty) = self.tables.annotation_type.get(named_type).copied()
            && let TypeKind::Interface { element, .. } = *self.ctx.ty(ty)
            && matches!(element.raw().tag(), Tag::Enum | Tag::Mixin)
        {
            // We have already reported the error.
            return;
        }
        let class_name = self.ast.qualified_name(named_type);
        // report as named or default constructor absence
        match self.ast[constructor_name].name {
            Some(name) => {
                let d =
                    diag::new_with_undefined_constructor(&class_name, self.identifier_lexeme(name));
                self.report_at(d, name);
            }
            None => {
                self.report_at(
                    diag::new_with_undefined_constructor_default(&class_name),
                    constructor_name,
                );
            }
        }
    }

    /// Dart `_checkForNonConstGenerativeEnumConstructor`.
    fn check_for_non_const_generative_enum_constructor(
        &mut self,
        node: Id<ConstructorDeclaration>,
    ) {
        let Some(element) = self.declared_element_of(node) else {
            return;
        };
        let in_enum = self
            .enclosing_interface_of(element)
            .is_some_and(|e| e.raw().tag() == Tag::Enum);
        if in_enum && !self.constructor_is_factory(element) && !self.constructor_is_const(element) {
            let range = self.constructor_error_range(node);
            self.report_at_range(diag::non_const_generative_enum_constructor(), range);
        }
    }

    /// Dart `_checkForNonFinalFieldInEnum`.
    fn check_for_non_final_field_in_enum(
        &mut self,
        field_declaration: Option<Id<FieldDeclaration>>,
        primary_constructor: Option<Id<PrimaryConstructorDeclaration>>,
    ) {
        if !self
            .enclosing_class
            .is_some_and(|c| c.raw().tag() == Tag::Enum)
        {
            return;
        }
        if let Some(field_declaration) = field_declaration {
            let n = &self.ast[field_declaration];
            // External fields do not add stored state to the enum instance.
            if n.static_keyword.is_none() && n.external_keyword.is_none() {
                let variable_list = n.fields;
                if !self.variable_list_is_final(variable_list)
                    && let Some(&first) = self.ast.list(self.ast[variable_list].variables).first()
                {
                    self.report_at_token(diag::non_final_field_in_enum(), self.ast[first].name);
                }
            }
        } else if let Some(primary_constructor) = primary_constructor {
            let list = self.ast[primary_constructor].formal_parameters;
            let parameters: Vec<NodeId> = self.ast.list_raw(self.ast[list].parameters).to_vec();
            for parameter in parameters {
                let Some(element) = self.declared_element_of(parameter) else {
                    continue;
                };
                if element.tag() != Tag::FieldFormalParameter
                    || !first_fragment_flags(&self.ctx, element)
                        .contains(FragmentFlags::FIELD_FORMAL_PARAMETER_FRAGMENT_IS_DECLARING)
                {
                    continue;
                }
                let name_token = formal_parameter_parts(self.ast, parameter).name;
                let field = self.field_formal_parameter_field(element);
                if let (Some(name_token), Some(field)) = (name_token, field)
                    && !first_fragment_flags(&self.ctx, field)
                        .contains(FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL)
                {
                    self.report_at_token(diag::non_final_field_in_enum(), name_token);
                }
            }
        }
    }

    /// Dart `_checkForNonRedirectingGenerativeConstructorWithPrimary`.
    fn check_for_non_redirecting_generative_constructor_with_primary(
        &mut self,
        node: Id<ConstructorDeclaration>,
    ) {
        let Some(fragment) = self.declared_fragment_of(node) else {
            return;
        };
        if self.fragment_has(fragment, FragmentFlags::FRAGMENT_IS_AUGMENTATION) {
            return;
        }
        let Some(enclosing_class) = self.enclosing_class else {
            return;
        };
        let has_primary = self
            .ctx
            .interface(enclosing_class)
            .constructors
            .iter()
            .any(|&c| {
                first_fragment_flags(&self.ctx, c.raw())
                    .contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_PRIMARY)
            });
        if enclosing_class.raw().tag() == Tag::ExtensionType || !has_primary {
            return;
        }
        let Some(element) = self.fragment_element(fragment) else {
            return;
        };
        if self.constructor_is_factory(element) || self.constructor_is_redirecting(element) {
            return;
        }
        let range = self.constructor_error_range(node);
        self.report_at_range(
            diag::non_redirecting_generative_constructor_with_primary(),
            range,
        );
    }

    /// Dart `_checkForRecursiveConstructorRedirect`.
    fn check_for_recursive_constructor_redirect(
        &mut self,
        declaration: Id<ConstructorDeclaration>,
        constructor_element: ElementId,
    ) {
        // we check generative constructor here
        if self.ast[declaration].factory_keyword.is_some() {
            return;
        }
        // try to find redirecting constructor invocation and analyze it for
        // recursion
        let initializers = self.ast[declaration].initializers;
        let redirecting = self
            .ast
            .list_raw(initializers)
            .iter()
            .copied()
            .find(|&i| self.ast.is::<RedirectingConstructorInvocation>(i));
        if let Some(initializer) = redirecting
            && self.has_redirecting_factory_constructor_cycle(constructor_element)
        {
            self.report_at(diag::recursive_constructor_redirect(), initializer);
        }
    }

    /// Dart `_checkForRecursiveFactoryRedirect`: whether the constructor
    /// [declaration] has a redirected constructor and references itself
    /// directly or indirectly (reports it).
    fn check_for_recursive_factory_redirect(
        &mut self,
        declaration: Id<ConstructorDeclaration>,
        element: ElementId,
    ) -> bool {
        // prepare redirected constructor
        let Some(redirected_constructor_node) = self.ast[declaration].redirected_constructor else {
            return false;
        };
        // OK if no cycle
        if !self.has_redirecting_factory_constructor_cycle(element) {
            return false;
        }
        // report error
        self.report_at(
            diag::recursive_factory_redirect(),
            redirected_constructor_node,
        );
        true
    }

    /// Dart `_checkForRedirectingConstructorErrorCodes`.
    fn check_for_redirecting_constructor_error_codes(
        &mut self,
        declaration: Id<ConstructorDeclaration>,
    ) {
        // Check for default values in the parameters.
        let Some(redirected_constructor) = self.ast[declaration].redirected_constructor else {
            return;
        };
        let parameters = self.ast[self.ast[declaration].parameters].parameters;
        for &parameter in self.ast.list_raw(parameters) {
            let parts = formal_parameter_parts(self.ast, parameter);
            if parts.default_clause.is_some()
                && let Some(name) = parts.name
            {
                self.report_at_token(
                    diag::default_value_in_redirecting_factory_constructor(),
                    name,
                );
            }
        }
        let redirected_element = self.element(redirected_constructor);
        if let Some(element) = self.declared_element_of(declaration) {
            self.check_for_redirect_to_non_const_constructor(
                element,
                redirected_element,
                Entity::Node(redirected_constructor.raw()),
            );
        }
        if let Some(redirected) = redirected_element {
            let base = member::base_element(&self.ctx, redirected);
            if let Some(redirected_class) = self.enclosing_interface_of(base)
                && redirected_class.raw().tag() == Tag::Class
                && self.class_is_abstract(redirected_class.raw())
                && !self.constructor_is_factory(base)
            {
                let mut constructor_str_name = self
                    .enclosing_class
                    .map(|c| self.element_name(c.raw()))
                    .unwrap_or_default();
                if let Some(name) = self.ast[declaration].name {
                    constructor_str_name.push('.');
                    constructor_str_name.push_str(self.ast.tokens.lexeme(name));
                }
                let abstract_class = self.element_name(redirected_class.raw());
                self.report_at(
                    diag::redirect_to_abstract_class_constructor(
                        &constructor_str_name,
                        &abstract_class,
                    ),
                    redirected_constructor,
                );
            }
        }
        self.check_for_invalid_generative_constructor_reference(
            redirected_constructor.raw(),
            redirected_element,
        );
    }

    /// Dart `_checkForRedirectToNonConstConstructor`: reports when the
    /// redirecting constructor, [element], is const, and
    /// [redirected_element], its redirectee, is not const.
    fn check_for_redirect_to_non_const_constructor(
        &mut self,
        element: ElementId,
        redirected_element: Option<ElemRef>,
        error_entity: Entity,
    ) {
        // This constructor is const, but it redirects to a non-const
        // constructor.
        if let Some(redirected) = redirected_element {
            let base = member::base_element(&self.ctx, redirected);
            if base.tag() == Tag::Constructor
                && self.constructor_is_const(element)
                && !self.constructor_is_const(base)
            {
                self.report_at_entity(diag::redirect_to_non_const_constructor(), error_entity);
            }
        }
    }

    /// Dart `_checkForReturnInGenerativeConstructor`.
    fn check_for_return_in_generative_constructor(
        &mut self,
        declaration: Id<ConstructorDeclaration>,
    ) {
        // ignore factory
        if self.ast[declaration].factory_keyword.is_some() {
            return;
        }
        // block body (with possible return statement) is checked elsewhere
        let body = self.ast[declaration].body;
        if !self.ast.is::<ExpressionFunctionBody>(body) {
            return;
        }
        self.report_at(diag::return_in_generative_constructor(), body);
    }

    /// Dart `_checkForUndefinedConstructorInInitializerImplicit`.
    fn check_for_undefined_constructor_in_initializer_implicit(
        &mut self,
        formal_parameter_list: Id<FormalParameterList>,
        initializers: Option<NodeList<ConstructorInitializer>>,
        error_range: SourceRange,
    ) {
        // Ignore if the constructor has either an explicit super constructor
        // invocation or a redirecting constructor invocation.
        if let Some(initializers) = initializers
            && self.ast.list_raw(initializers).iter().any(|&i| {
                self.ast.is::<SuperConstructorInvocation>(i)
                    || self.ast.is::<RedirectingConstructorInvocation>(i)
            })
        {
            return;
        }

        // Check to see whether the superclass has a non-factory unnamed
        // constructor.
        let Some(enclosing_class) = self.enclosing_class else {
            return;
        };
        let ctx = self.ctx;
        let Some(super_type) = ctx.interface(enclosing_class).supertype.get() else {
            return;
        };
        let TypeKind::Interface {
            element: super_element,
            ..
        } = *ctx.ty(super_type)
        else {
            return;
        };

        let super_constructors = &ctx.interface(super_element).constructors;
        if super_constructors
            .iter()
            .all(|&c| self.constructor_is_factory(c.raw()))
        {
            // Already reported [NO_GENERATIVE_CONSTRUCTORS_IN_SUPERCLASS].
            return;
        }

        let super_unnamed_constructor = super_constructors
            .iter()
            .copied()
            .find(|&c| self.element_name(c.raw()) == "new");
        let Some(super_unnamed_constructor) = super_unnamed_constructor else {
            let class_name = self.element_name(super_element.raw());
            self.report_at_range(
                diag::undefined_constructor_in_initializer_default(&class_name),
                error_range,
            );
            return;
        };

        if self.constructor_is_factory(super_unnamed_constructor.raw()) {
            self.report_at_range(
                diag::non_generative_constructor(element_arg(
                    &ctx,
                    super_unnamed_constructor.raw(),
                )),
                error_range,
            );
            return;
        }

        let parameters = &ctx.get(super_unnamed_constructor).formal_params;
        let required_positional_parameter_count = parameters
            .iter()
            .filter(|&&p| ctx.get(p).kind.is_required_positional())
            .count();
        let mut required_named_parameters: IndexSet<String> = parameters
            .iter()
            .filter(|&&p| ctx.get(p).kind.is_required_named())
            .map(|&p| self.element_name(p.raw()))
            .collect();

        if !self.library_feature_enabled(ExperimentalFlag::SuperParameters) {
            if required_positional_parameter_count != 0 || !required_named_parameters.is_empty() {
                self.report_at_range(
                    diag::no_default_super_constructor_explicit(type_arg(&ctx, super_type)),
                    error_range,
                );
            }
            return;
        }

        let super_parameters_result =
            verify_super_formal_parameters(self, formal_parameter_list, true, false);
        for name in &super_parameters_result.named_argument_names {
            required_named_parameters.shift_remove(name);
        }

        if required_positional_parameter_count > super_parameters_result.positional_argument_count
            || !required_named_parameters.is_empty()
        {
            self.report_at_range(
                diag::implicit_super_initializer_missing_arguments(type_arg(&ctx, super_type)),
                error_range,
            );
        }
    }

    /// Dart `_checkForUndefinedConstructorInInitializerImplicitConstructor`:
    /// checks that if the generative [constructor] has neither an explicit
    /// super constructor invocation nor a redirecting constructor
    /// invocation, the superclass has a default generative constructor.
    fn check_for_undefined_constructor_in_initializer_implicit_constructor(
        &mut self,
        constructor: Id<ConstructorDeclaration>,
    ) {
        if self.enclosing_class.is_none() {
            return;
        }
        let n = &self.ast[constructor];
        // Ignore if the constructor is not generative.
        if n.factory_keyword.is_some() {
            return;
        }
        // Ignore if the constructor is external. See
        // https://github.com/dart-lang/language/issues/869.
        if n.external_keyword.is_some() {
            return;
        }
        let (parameters, initializers) = (n.parameters, n.initializers);
        let range = self.constructor_error_range(constructor);
        self.check_for_undefined_constructor_in_initializer_implicit(
            parameters,
            Some(initializers),
            range,
        );
    }

    /// Dart `_checkForValidField`.
    fn check_for_valid_field(&mut self, parameter: Id<FieldFormalParameter>) {
        let constructor = self
            .ast
            .parent(parameter)
            .and_then(|list| self.ast.parent(list));
        let Some(constructor) = constructor else {
            return;
        };
        if self.ast.is::<PrimaryConstructorDeclaration>(constructor)
            && self
                .ast
                .parent(constructor)
                .is_some_and(|p| self.ast.is::<ExtensionTypeDeclaration>(p))
        {
            return;
        }
        if !self.ast.is::<ConstructorDeclaration>(constructor)
            && !self.ast.is::<PrimaryConstructorDeclaration>(constructor)
        {
            return;
        }

        let Some(element) = self.declared_element_of(parameter) else {
            return;
        };
        if element.tag() != Tag::FieldFormalParameter {
            return;
        }

        let name = self.ast[parameter].name;
        let field_element = self.field_formal_parameter_field(element).filter(|&f| {
            !first_fragment_flags(&self.ctx, f)
                .contains(FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_GETTER_SETTER)
        });
        let Some(field_element) = field_element else {
            let d = diag::initializing_formal_for_non_existent_field(self.ast.tokens.lexeme(name));
            self.report_at(d, parameter);
            return;
        };

        if first_fragment_flags(&self.ctx, field_element)
            .contains(FragmentFlags::VARIABLE_FRAGMENT_IS_STATIC)
        {
            let d = diag::initializer_for_static_field(self.ast.tokens.lexeme(name));
            self.report_at_token(d, self.ast[parameter].this_keyword);
            return;
        }

        let ctx = self.ctx;
        let element_type = variable_type(&ctx, element);
        let field_type = variable_type(&ctx, field_element);
        if !self.type_system.is_subtype_of(element_type, field_type) {
            self.report_at(
                diag::field_initializing_formal_not_assignable(
                    type_arg(&ctx, element_type),
                    type_arg(&ctx, field_type),
                ),
                parameter,
            );
        }
    }

    /// Dart `_checkForWrongTypeParameterVarianceInField`.
    fn check_for_wrong_type_parameter_variance_in_field(&mut self, node: Id<FieldDeclaration>) {
        let Some(enclosing_class) = self.enclosing_class else {
            return;
        };
        let ctx = self.ctx;
        let fields = self.ast[node].fields;
        let Some(&first) = self.ast.list(self.ast[fields].variables).first() else {
            return;
        };
        let type_parameters = ctx.interface(enclosing_class).type_params.clone();
        for type_parameter in type_parameters {
            // Dart `isLegacyCovariant`.
            if ctx.get(type_parameter).variance.is_none() {
                continue;
            }
            let Some(field_element) = self.declared_element_of(first) else {
                return;
            };
            let field_name = self.ast[first].name;
            let field_variance =
                compute_variance_in_type(&ctx, type_parameter, variable_type(&ctx, field_element));
            self.check_for_wrong_variance_position(
                to_element_variance(field_variance),
                type_parameter.raw(),
                Entity::Token(field_name),
            );
            if !self.variable_list_is_final(fields) && self.ast[node].covariant_keyword.is_none() {
                self.check_for_wrong_variance_position(
                    to_element_variance(SharedVariance::Contravariant.combine(field_variance)),
                    type_parameter.raw(),
                    Entity::Token(field_name),
                );
            }
        }
    }

    /// Dart `_hasRedirectingFactoryConstructorCycle`.
    fn has_redirecting_factory_constructor_cycle(&self, constructor: ElementId) -> bool {
        let mut constructors: IndexSet<ElementId> = IndexSet::new();
        let mut current = Some(constructor);
        while let Some(c) = current {
            if constructors.contains(&c) {
                return c == constructor;
            }
            constructors.insert(c);
            current = self
                .redirected_constructor(c)
                .map(|r| member::base_element(&self.ctx, r));
        }
        false
    }

    /// Dart `_validateConstructorBodyAllowed`.
    fn validate_constructor_body_allowed(
        &mut self,
        element: ElementId,
        const_keyword: Option<TokenId>,
        external_keyword: Option<TokenId>,
        is_redirecting: bool,
        body: Id<FunctionBody>,
        is_primary: bool,
    ) {
        let block_body = self.ast.cast::<BlockFunctionBody>(body);
        let expression_body = self.ast.cast::<ExpressionFunctionBody>(body);
        // The token of the body that the errors are reported at.
        let body_token = if let Some(b) = block_body {
            Some(self.ast[self.ast[b].block].left_bracket)
        } else {
            expression_body.map(|b| self.ast[b].function_definition)
        };
        if self.constructor_is_factory(element) {
            if external_keyword.is_some() {
                if let Some(token) = body_token {
                    self.report_at_token(diag::external_factory_with_body(), token);
                }
            } else if let Some(const_keyword) = const_keyword
                && !is_redirecting
            {
                self.report_at_token(diag::const_factory(), const_keyword);
            }
        } else {
            self.check_for_external_method_with_body(external_keyword, body);
            if is_redirecting && let Some(token) = body_token {
                self.report_at_token(diag::redirecting_constructor_with_body(), token);
            }
            if self.constructor_is_const(element) {
                if let Some(token) = body_token {
                    let d = if !is_primary {
                        diag::const_constructor_with_body()
                    } else if block_body.is_some() {
                        diag::const_primary_constructor_with_block_body()
                    } else {
                        diag::const_primary_constructor_with_expression_body()
                    };
                    self.report_at_token(d, token);
                }
            } else if is_primary && let Some(b) = expression_body {
                self.report_at_token(
                    diag::primary_constructor_body_with_expression_body(),
                    self.ast[b].function_definition,
                );
            }
        }
    }

    // ------------------------------------------------------------ element and AST getters

    /// Whether the resolver resolved the subtree of [node]: an expression
    /// in it has a static type (the scope pass already sets the elements of
    /// some identifiers and named types).
    fn subtree_is_resolved(&self, node: NodeId) -> bool {
        let mut stack = vec![node];
        while let Some(n) = stack.pop() {
            if self.static_type(n).is_some() {
                return true;
            }
            stack.extend(self.ast.children(n));
        }
        false
    }

    /// Not in Dart: whether [node] is in an annotation that the resolver did
    /// not resolve (`annotation_resolver.rs` is a stub of unit C9). The
    /// checks of the nodes in it would report false positives
    /// (`const_with_undefined_constructor_default`, ...).
    fn is_in_unresolved_annotation(&self, node: NodeId) -> bool {
        self.ast
            .this_or_ancestor_of_type::<Annotation>(node)
            .is_some_and(|a| !self.subtree_is_resolved(a.raw()))
    }

    /// Dart `node.declaredFragment`.
    fn declared_fragment_of(&self, node: impl Into<NodeId>) -> Option<FragmentId> {
        self.tables.declared_fragment.get(node.into()).copied()
    }

    /// Dart `fragment.element`.
    fn fragment_element(&self, fragment: FragmentId) -> Option<ElementId> {
        self.ctx.fragment_data(fragment)?.element.try_get().copied()
    }

    /// Dart `node.declaredFragment?.element`.
    fn declared_element_of(&self, node: impl Into<NodeId>) -> Option<ElementId> {
        self.fragment_element(self.declared_fragment_of(node)?)
    }

    /// Dart `fragment.previousFragment`.
    fn previous_fragment(&self, fragment: FragmentId) -> Option<FragmentId> {
        self.ctx.fragment_data(fragment)?.previous_fragment
    }

    /// Dart `element.fragments`.
    fn element_fragments(&self, element: ElementId) -> Vec<FragmentId> {
        let mut out = Vec::new();
        let mut fragment = self.ctx.element_data(element).map(|d| d.first_fragment);
        while let Some(f) = fragment {
            out.push(f);
            fragment = self.ctx.fragment_data(f).and_then(|d| d.next_fragment);
        }
        out
    }

    /// Dart `FragmentImpl.nearestPrecedingCompleteFragment`.
    fn nearest_preceding_complete_fragment(&self, fragment: FragmentId) -> Option<FragmentId> {
        let mut current = self.previous_fragment(fragment);
        while let Some(f) = current {
            if self.fragment_has(f, FragmentFlags::FRAGMENT_IS_COMPLETE) {
                return Some(f);
            }
            current = self.previous_fragment(f);
        }
        None
    }

    /// Dart `PropertyInducingFragmentImpl.inducedGetter`.
    fn induced_getter(&self, fragment: FragmentId) -> Option<FragmentId> {
        let element = self.fragment_element(fragment)?;
        let getter = self.ctx.property_inducing(element.cast()?).getter?;
        self.induced_accessor_fragment(fragment, getter.raw())
    }

    /// Dart `PropertyInducingFragmentImpl.inducedSetter`.
    fn induced_setter(&self, fragment: FragmentId) -> Option<FragmentId> {
        let element = self.fragment_element(fragment)?;
        let setter = self.ctx.property_inducing(element.cast()?).setter?;
        self.induced_accessor_fragment(fragment, setter.raw())
    }

    /// The fragment of [accessor] that the variable [fragment] induces: the
    /// accessor fragment with the same name offset (an induced accessor
    /// fragment has the offset of its variable).
    fn induced_accessor_fragment(
        &self,
        fragment: FragmentId,
        accessor: ElementId,
    ) -> Option<FragmentId> {
        let offset = self.ctx.fragment_data(fragment)?.name_offset;
        self.element_fragments(accessor).into_iter().find(|&f| {
            self.ctx.fragment_data(f).is_some_and(|d| {
                d.name_offset == offset
                    && d.flags
                        .has(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE)
            })
        })
    }

    /// The name of [element] (Dart `name ?? ''`).
    fn element_name(&self, element: ElementId) -> String {
        self.ctx
            .element_data(element)
            .and_then(|d| d.name)
            .map(|n| self.ctx.name_str(n).to_string())
            .unwrap_or_default()
    }

    /// The lexeme of [identifier].
    fn identifier_lexeme(&self, identifier: Id<SimpleIdentifier>) -> &str {
        self.ast.tokens.lexeme(self.ast[identifier].token)
    }

    /// Dart `element.enclosingElement` as an interface element.
    fn enclosing_interface_of(&self, element: ElementId) -> Option<EId<InterfaceElement>> {
        self.ctx.element_data(element)?.enclosing?.cast()
    }

    /// Dart `ConstructorElement.isConst`.
    fn constructor_is_const(&self, element: ElementId) -> bool {
        first_fragment_flags(&self.ctx, element)
            .contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_CONST)
    }

    /// Dart `ConstructorElement.isFactory`.
    fn constructor_is_factory(&self, element: ElementId) -> bool {
        first_fragment_flags(&self.ctx, element)
            .contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY)
    }

    /// Dart `ConstructorElementImpl.isRedirecting`.
    fn constructor_is_redirecting(&self, element: ElementId) -> bool {
        self.element_fragments(element)
            .iter()
            .any(|&f| self.fragment_has(f, FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_REDIRECTING))
    }

    /// Dart `ClassElement.isAbstract`.
    fn class_is_abstract(&self, element: ElementId) -> bool {
        self.ctx
            .element_data(element)
            .is_some_and(|d| d.flags.has(ElementFlags::CLASS_ELEMENT_IS_ABSTRACT))
    }

    /// Dart `ConstructorElement.redirectedConstructor`.
    fn redirected_constructor(&self, element: ElementId) -> Option<ElemRef> {
        let constructor = element.cast::<ConstructorElement>()?;
        self.ctx.get(constructor).redirected_constructor.get()
    }

    /// Dart `ConstructorElement.superConstructor`: the constructor of the
    /// supertype (Dart `InterfaceType.getNamedConstructor`, substituted with
    /// the type arguments of the supertype).
    fn super_constructor(&self, element: ElementId) -> Option<ElemRef> {
        let constructor = element.cast::<ConstructorElement>()?;
        let super_constructor = self.ctx.get(constructor).super_constructor.get()?;
        match super_constructor {
            ElemRef::Base(base) => {
                let enclosing = self.enclosing_interface_of(element)?;
                match self.ctx.interface(enclosing).supertype.get() {
                    Some(supertype) => Some(member::constructor_from2(&self.ctx, base, supertype)),
                    None => Some(super_constructor),
                }
            }
            ElemRef::Member(_) => Some(super_constructor),
        }
    }

    /// Dart `SuperFormalParameterElementImpl.superConstructorParameter`.
    fn super_constructor_parameter(&self, element: ElementId) -> Option<ElemRef> {
        let ctx = self.ctx;
        let parameter = element.cast::<FormalParameterElement>()?;
        let constructor = ctx.element_data(element)?.enclosing?;
        let super_constructor = self.super_constructor(constructor)?;
        let super_parameters = member::formal_parameters(&ctx, super_constructor);
        let kind_of = |p: ElemRef| -> Option<ParameterKind> {
            let base = member::base_element(&ctx, p).cast::<FormalParameterElement>()?;
            Some(ctx.get(base).kind)
        };
        let pe = ctx.get(parameter);
        if pe.kind.is_named() {
            super_parameters.iter().copied().find(|&s| {
                kind_of(s).is_some_and(|k| k.is_named())
                    && member::name(&ctx, s) == pe.name.map(|n| ctx.name_str(n))
            })
        } else {
            // Dart `indexIn(enclosingElement)`.
            let constructor = constructor.cast::<ConstructorElement>()?;
            let index = ctx
                .get(constructor)
                .formal_params
                .iter()
                .copied()
                .filter(|x| {
                    x.raw().tag() == Tag::SuperFormalParameter && ctx.get(*x).kind.is_positional()
                })
                .position(|x| x == parameter)?;
            super_parameters
                .iter()
                .copied()
                .filter(|&s| kind_of(s).is_some_and(|k| k.is_positional()))
                .nth(index)
        }
    }

    /// Dart `FieldFormalParameterElement.field`.
    fn field_formal_parameter_field(&self, element: ElementId) -> Option<ElementId> {
        let parameter = element.cast::<FormalParameterElement>()?;
        self.ctx.get(parameter).field.get().map(|f| f.raw())
    }

    /// Dart `VariableDeclarationList.isConst`.
    fn variable_list_is_const(&self, list: Id<VariableDeclarationList>) -> bool {
        crate::ast_ext::is_keyword(self.ast, self.ast[list].keyword, "const")
    }

    /// Dart `VariableDeclarationList.isFinal`.
    fn variable_list_is_final(&self, list: Id<VariableDeclarationList>) -> bool {
        crate::ast_ext::is_keyword(self.ast, self.ast[list].keyword, "final")
    }

    /// Dart `DotShorthandConstructorInvocation.isConst`.
    fn dot_shorthand_constructor_invocation_is_const(
        &self,
        node: Id<DotShorthandConstructorInvocation>,
    ) -> bool {
        crate::ast_ext::is_keyword(self.ast, self.ast[node].const_keyword, "const")
            || crate::ast_ext::in_constant_context(self.ast, node.raw())
    }

    /// Dart `_featureSet.isEnabled(feature)` (the features of the unit).
    fn unit_feature_enabled(&self, flag: ExperimentalFlag) -> bool {
        self.unit.features.is_experiment_enabled(flag)
    }

    /// Dart `_currentLibrary.featureSet.isEnabled(feature)`.
    fn library_feature_enabled(&self, flag: ExperimentalFlag) -> bool {
        crate::scope::library_feature_enabled(&self.ctx, self.current_library(), flag)
    }

    /// Dart `ConstructorDeclaration.errorRange`.
    fn constructor_error_range(&self, node: Id<ConstructorDeclaration>) -> SourceRange {
        let n = &self.ast[node];
        let start_entity = match n.type_name {
            Some(t) => Some(Entity::Node(t.raw())),
            None => n.new_keyword.or(n.factory_keyword).map(Entity::Token),
        };
        let Some(start_entity) = start_entity else {
            return self.node_range(node.raw());
        };
        let end_entity = n.name.map(Entity::Token).unwrap_or(start_entity);
        let offset = self.ast.entity_offset(start_entity);
        let end = self.entity_end(end_entity);
        SourceRange {
            offset,
            length: end.saturating_sub(offset),
        }
    }

    /// Dart `PrimaryConstructorDeclaration.errorRange`.
    fn primary_constructor_error_range(
        &self,
        node: Id<PrimaryConstructorDeclaration>,
    ) -> SourceRange {
        let begin = self.ast.begin_token(node);
        let offset = self.ast.tokens.offset(begin);
        let end = match self.ast[node].constructor_name {
            Some(name) => self.ast.end(name),
            None => self.ast.tokens.get(begin).end(),
        };
        SourceRange {
            offset,
            length: end.saturating_sub(offset),
        }
    }

    /// The end offset of [entity].
    fn entity_end(&self, entity: Entity) -> u32 {
        match entity {
            Entity::Node(n) => self.ast.end(n),
            Entity::Token(t) => self.ast.tokens.get(t).end(),
        }
    }

    /// Dart `node.sourceRange`.
    fn node_range(&self, node: NodeId) -> SourceRange {
        SourceRange {
            offset: self.ast.offset(node),
            length: self.ast.length(node),
        }
    }

    /// Dart `token.sourceRange`.
    fn token_range(&self, token: TokenId) -> SourceRange {
        let t = self.ast.tokens.get(token);
        SourceRange {
            offset: t.offset,
            length: t.end() - t.offset,
        }
    }

    /// Dart `diagnostic.atSourceRange(range)`.
    fn report_at_range(&mut self, diagnostic: LocatableDiagnostic, range: SourceRange) {
        let d = diagnostic.at_offset(range.offset as usize, range.length as usize);
        self.report(d);
    }

    /// Dart `diagnostic.at(entity)` for a syntactic entity.
    fn report_at_entity(&mut self, diagnostic: LocatableDiagnostic, entity: Entity) {
        match entity {
            Entity::Node(n) => self.report_at(diagnostic, n),
            Entity::Token(t) => self.report_at_token(diagnostic, t),
        }
    }

    // Helpers that other sections call.

    /// Dart `_checkForAugmentationReturnTypeMismatch(fragment:,
    /// returnTypeNode:, errorEntity:)`.
    pub(crate) fn check_for_augmentation_return_type_mismatch(
        &mut self,
        fragment: FragmentId,
        return_type_node: Option<Id<TypeAnnotation>>,
        error_entity: Entity,
    ) {
        if !self.fragment_has(fragment, FragmentFlags::FRAGMENT_IS_AUGMENTATION) {
            return;
        }
        let Some(return_type_node) = return_type_node else {
            return;
        };
        let Some(element) = self.fragment_element(fragment) else {
            return;
        };
        let ctx = self.ctx;
        let expected_type = member::return_type(&ctx, ElemRef::Base(element));
        let Some(actual_type) = self.tables.annotation_type.get(return_type_node).copied() else {
            return;
        };
        if actual_type == TypeId::INVALID || expected_type == TypeId::INVALID {
            return;
        }
        if actual_type == expected_type {
            return;
        }
        // Dart `fragment is GetterFragmentImpl && fragment.inducingVariable
        // != null`.
        let is_induced_getter = fragment.tag() == Tag::Getter
            && self.fragment_has(
                fragment,
                FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE,
            );
        let (expected, actual) = (type_arg(&ctx, expected_type), type_arg(&ctx, actual_type));
        let d = if is_induced_getter {
            diag::augmentation_induced_getter_return_type_mismatch(expected, actual)
        } else {
            diag::augmentation_return_type_mismatch(expected, actual)
        };
        self.report_at_entity(d, error_entity);
    }

    /// Dart `_checkForConstVariableAugmentation(errorToken:, fragment:)`.
    pub(crate) fn check_for_const_variable_augmentation(
        &mut self,
        error_token: TokenId,
        fragment: FragmentId,
    ) -> bool {
        if !self.fragment_has(fragment, FragmentFlags::FRAGMENT_IS_AUGMENTATION) {
            return false;
        }
        if self.fragment_has(fragment, FragmentFlags::VARIABLE_FRAGMENT_IS_CONST) {
            self.report_at_token(diag::constant_variable_augmentation(), error_token);
            return true;
        }
        if let Some(element) = self.fragment_element(fragment)
            && first_fragment_flags(&self.ctx, element)
                .contains(FragmentFlags::VARIABLE_FRAGMENT_IS_CONST)
        {
            self.report_at_token(diag::augments_constant_variable(), error_token);
            return true;
        }
        false
    }

    /// Dart `_checkForDefaultValueAssignableAtType(node)`.
    pub(crate) fn check_for_default_value_assignable_at_type(&mut self, node: Id<FormalParameter>) {
        let parts = formal_parameter_parts(self.ast, node.raw());
        let Some(default_clause) = parts.default_clause else {
            return;
        };
        let default_value = self.ast[default_clause].value;
        let Some(element) = self.declared_element_of(node) else {
            return;
        };
        let Some(actual) = self.static_type(default_value) else {
            return;
        };
        let expected = variable_type(&self.ctx, element);
        self.check_for_assignable_expression_at_type(
            default_value,
            actual,
            expected,
            NonAssignabilityReporter::ForAssignment,
        );
    }

    /// Dart `_checkForInvalidGenerativeConstructorReference(node,
    /// constructorElement)`: verifies that an enum constructor is used only
    /// to create an enum constant or as a target of constructor redirection.
    pub(crate) fn check_for_invalid_generative_constructor_reference(
        &mut self,
        node: NodeId,
        constructor_element: Option<ElemRef>,
    ) {
        let Some(constructor_element) = constructor_element else {
            return;
        };
        let base = member::base_element(&self.ctx, constructor_element);
        if base.tag() != Tag::Constructor || self.constructor_is_factory(base) {
            return;
        }
        if !self
            .enclosing_interface_of(base)
            .is_some_and(|e| e.raw().tag() == Tag::Enum)
        {
            return;
        }
        if self.library_feature_enabled(ExperimentalFlag::EnhancedEnums) {
            let is_tear_off = self.ast.parent(node).is_some_and(|p| {
                self.ast.is::<ConstructorReference>(p)
                    && !self
                        .ast
                        .parent(p)
                        .is_some_and(|g| self.ast.is::<InstanceCreationExpression>(g))
            });
            if is_tear_off {
                self.report_at(
                    diag::invalid_reference_to_generative_enum_constructor_tearoff(),
                    node,
                );
            } else {
                self.report_at(
                    diag::invalid_reference_to_generative_enum_constructor(),
                    node,
                );
            }
        } else {
            self.report_at(diag::instantiate_enum(), node);
        }
    }

    /// Dart `_checkForWrongVariancePosition(variance, typeParameter,
    /// errorTarget)`.
    pub(crate) fn check_for_wrong_variance_position(
        &mut self,
        variance: Variance,
        type_parameter: ElementId,
        error_target: Entity,
    ) {
        let Some(type_parameter) = type_parameter.cast::<TypeParameterElement>() else {
            return;
        };
        let declared = type_parameter_variance(&self.ctx, type_parameter);
        let variance = to_shared_variance(variance);
        if !variance.greater_than_or_equal(declared) {
            let name = self.element_name(type_parameter.raw());
            self.report_at_entity(
                diag::wrong_type_parameter_variance_position(
                    declared.keyword(),
                    &name,
                    variance.keyword(),
                ),
                error_target,
            );
        }
    }

    /// Dart `_checkPrivateOptionalParameter(node)`.
    pub(crate) fn check_private_optional_parameter(&mut self, node: Id<FormalParameter>) {
        let parts = formal_parameter_parts(self.ast, node.raw());
        // Must be a named parameter.
        if !parts.kind.is_named() {
            return;
        }
        // Must be private.
        let Some(name) = parts.name else {
            return;
        };
        let lexeme = self.ast.tokens.lexeme(name).to_string();
        if crate::ast_ext::token_is_synthetic(self.ast, name) || !lexeme.starts_with('_') {
            return;
        }

        let feature = ExperimentalFlag::PrivateNamedParameters;
        if !self.library_feature_enabled(feature) {
            if self.ast.is::<FieldFormalParameter>(node) {
                // The user is using syntax that is now meaningful, but in a
                // library where it isn't enabled, so report a more precise
                // error.
                if feature.is_enabled_by_default() {
                    let (major, minor) = feature.experiment_released_version();
                    let version = format!("{major}.{minor}.0");
                    self.report_at_token(
                        diag::experiment_not_enabled(feature.name(), &version),
                        name,
                    );
                } else {
                    self.report_at_token(
                        diag::experiment_not_enabled_off_by_default(feature.name()),
                        name,
                    );
                }
            } else {
                self.report_at_token(diag::private_optional_parameter(), name);
            }
            return;
        }

        // Must refer to a field.
        let element = self.declared_element_of(node);
        if !element.is_some_and(|e| e.tag() == Tag::FieldFormalParameter) {
            self.report_at_token(diag::private_named_non_field_parameter(), name);
            return;
        }

        if corresponding_public_name(&lexeme).is_none() {
            self.report_at_token(diag::private_named_parameter_without_public_name(), name);
        }
    }
}

/// Dart `PrimaryConstructorDeclarationImpl.body`: the first
/// `PrimaryConstructorBody` of the members of the enclosing declaration.
fn primary_constructor_declaration_body(
    ast: &Ast,
    node: Id<PrimaryConstructorDeclaration>,
) -> Option<Id<PrimaryConstructorBody>> {
    let parent = ast.parent(node)?;
    let body: NodeId = if let Some(c) = ast.cast::<ClassDeclaration>(parent) {
        ast[c].body.raw()
    } else if let Some(e) = ast.cast::<EnumDeclaration>(parent) {
        ast[e].body.raw()
    } else {
        ast[ast.cast::<ExtensionTypeDeclaration>(parent)?]
            .body
            .raw()
    };
    let members = if let Some(b) = ast.cast::<BlockClassBody>(body) {
        ast[b].members
    } else {
        ast[ast.cast::<BlockEnumBody>(body)?].members
    };
    ast.list_raw(members)
        .iter()
        .find_map(|&m| ast.cast::<PrimaryConstructorBody>(m))
}

/// The return type of the function type [ty].
fn function_return_type(ctx: &dartr_element::Ctx<'_>, ty: TypeId) -> Option<TypeId> {
    match ctx.ty(ty) {
        TypeKind::Function(f) => Some(f.ret),
        _ => None,
    }
}

/// Dart `TypeParameterElementImpl.variance` (`_variance ?? covariant`).
fn type_parameter_variance(
    ctx: &dartr_element::Ctx<'_>,
    type_parameter: EId<TypeParameterElement>,
) -> SharedVariance {
    ctx.get(type_parameter)
        .variance
        .map(to_shared_variance)
        .unwrap_or(SharedVariance::Covariant)
}

fn to_shared_variance(v: Variance) -> SharedVariance {
    match v {
        Variance::Unrelated => SharedVariance::Unrelated,
        Variance::Covariant => SharedVariance::Covariant,
        Variance::Contravariant => SharedVariance::Contravariant,
        Variance::Invariant => SharedVariance::Invariant,
    }
}

fn to_element_variance(v: SharedVariance) -> Variance {
    match v {
        SharedVariance::Unrelated => Variance::Unrelated,
        SharedVariance::Covariant => Variance::Covariant,
        SharedVariance::Contravariant => Variance::Contravariant,
        SharedVariance::Invariant => Variance::Invariant,
    }
}

/// Dart `TypeParameterElementImpl.computeVarianceInType(type)`: the
/// variance of [type_parameter] in [ty].
pub(crate) fn compute_variance_in_type(
    ctx: &dartr_element::Ctx<'_>,
    type_parameter: EId<TypeParameterElement>,
    ty: TypeId,
) -> SharedVariance {
    match *ctx.ty(ty) {
        TypeKind::TypeParameter { param, .. } => {
            if param == type_parameter {
                SharedVariance::Covariant
            } else {
                SharedVariance::Unrelated
            }
        }
        TypeKind::Interface { element, args, .. } => {
            let mut result = SharedVariance::Unrelated;
            let parameters = &ctx.interface(element).type_params;
            for (i, &argument) in ctx.list(args).iter().enumerate() {
                let Some(&parameter) = parameters.get(i) else {
                    break;
                };
                let parameter_variance = type_parameter_variance(ctx, parameter);
                result = result.meet(parameter_variance.combine(compute_variance_in_type(
                    ctx,
                    type_parameter,
                    argument,
                )));
            }
            result
        }
        TypeKind::Function(f) => {
            let mut result = compute_variance_in_type(ctx, type_parameter, f.ret);
            for &parameter in ctx.list(f.type_params) {
                // If [parameter] is referenced in the bound at all, it makes
                // the variance of [parameter] in the entire type invariant.
                if let Some(bound) = ctx.get(parameter).bound.get()
                    && !compute_variance_in_type(ctx, type_parameter, bound).is_unrelated()
                {
                    result = SharedVariance::Invariant;
                }
            }
            for parameter in ctx.list(f.params) {
                result = result.meet(
                    SharedVariance::Contravariant.combine(compute_variance_in_type(
                        ctx,
                        type_parameter,
                        parameter.ty,
                    )),
                );
            }
            result
        }
        _ => SharedVariance::Unrelated,
    }
}

/// Dart `correspondingPublicName` (`_fe_analyzer_shared` token_impl.dart).
fn corresponding_public_name(identifier: &str) -> Option<String> {
    const RESERVED: &[&str] = &[
        "assert", "break", "case", "catch", "class", "const", "continue", "default", "do", "else",
        "enum", "extends", "false", "final", "finally", "for", "if", "in", "is", "new", "null",
        "rethrow", "return", "super", "switch", "this", "throw", "true", "try", "var", "void",
        "while", "with",
    ];
    let bytes = identifier.as_bytes();
    if bytes.first() != Some(&b'_') || bytes.len() == 1 {
        return None;
    }
    if bytes[1] == b'_' || bytes[1].is_ascii_digit() {
        return None;
    }
    let public = &identifier[1..];
    if RESERVED.contains(&public) {
        return None;
    }
    Some(public.to_string())
}

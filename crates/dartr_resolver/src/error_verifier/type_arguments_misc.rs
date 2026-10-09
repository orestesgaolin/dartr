// Dart source: pkg/analyzer/lib/src/generated/error_verifier.dart
// (ErrorVerifier section: type arguments, type parameters, formal parameters,
// functions and methods, variance, operators (D7))

//! An `ErrorVerifier` section (see the module documentation of
//! [`super`]). The `visit_x` methods are the Dart `visitX` overrides; the
//! body `self.visit_children(node)` is Dart `super.visitX(node)`.

use dartr_ast::*;
use dartr_element::{
    ElemRef, ElementId, ExecutableElement, FragmentFlags, FragmentId, InterfaceFragment, Tag,
    TypeId,
};
use dartr_syntax::TokenId;

use super::EnclosingExecutableContext;

impl EnclosingExecutableContext {
    /// Dart `EnclosingExecutableContext(element, isAsynchronous:,
    /// isGenerator:)`. The callers set `catch_error_on_error_return_type`
    /// and `then_on_error_return_type` after the construction.
    pub fn new(
        ctx: &dartr_element::Ctx<'_>,
        element: Option<ElementId>,
        is_asynchronous: bool,
        is_generator: bool,
    ) -> EnclosingExecutableContext {
        let is_constructor = element.is_some_and(|e| e.tag() == Tag::Constructor);
        let first_flags = |e: ElementId| crate::element_ext::first_fragment_flags(ctx, e);
        EnclosingExecutableContext {
            element,
            is_asynchronous,
            is_const_constructor: is_constructor
                && element.is_some_and(|e| {
                    first_flags(e).contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_CONST)
                }),
            is_generative_constructor: is_constructor
                && element.is_some_and(|e| {
                    !first_flags(e).contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY)
                }),
            is_generator,
            in_factory_constructor: Self::in_factory_constructor(ctx, element),
            in_static_method: Self::in_static_method(ctx, element),
            ..EnclosingExecutableContext::empty()
        }
    }

    /// Dart `displayName` of the element.
    pub fn display_name(&self, ctx: &dartr_element::Ctx<'_>) -> Option<String> {
        let e = self.element?;
        Some(executable_display_name(ctx, e))
    }

    /// Dart `isClosure`: a local function without a name.
    pub fn is_closure(&self, ctx: &dartr_element::Ctx<'_>) -> bool {
        match self.element {
            Some(e) if e.tag() == Tag::LocalFunction => {
                ctx.element_data(e).is_none_or(|d| d.name.is_none())
            }
            _ => false,
        }
    }

    /// Dart `isConstructor`.
    pub fn is_constructor(&self) -> bool {
        self.element.is_some_and(|e| e.tag() == Tag::Constructor)
    }

    /// Dart `isFunction`: a named local or top-level function, or a
    /// property accessor.
    pub fn is_function(&self, ctx: &dartr_element::Ctx<'_>) -> bool {
        match self.element {
            Some(e) => match e.tag() {
                Tag::LocalFunction | Tag::TopLevelFunction => {
                    !executable_display_name(ctx, e).is_empty()
                }
                Tag::Getter | Tag::Setter => true,
                _ => false,
            },
            None => false,
        }
    }

    /// Dart `isMethod`.
    pub fn is_method(&self) -> bool {
        self.element.is_some_and(|e| e.tag() == Tag::Method)
    }

    /// Dart `returnType`: `catchErrorOnErrorReturnType ??
    /// thenOnErrorReturnType ?? element!.returnType`. `None` when there is
    /// no element (Dart throws).
    pub fn return_type(&self, ctx: &dartr_element::Ctx<'_>) -> Option<TypeId> {
        self.catch_error_on_error_return_type
            .or(self.then_on_error_return_type)
            .or_else(|| {
                let e = self.element?;
                Some(dartr_typesystem::member::return_type(ctx, ElemRef::Base(e)))
            })
    }

    /// The element of the enclosing fragment of the first fragment of
    /// [element] (Dart `element?.firstFragment.enclosingFragment`), with
    /// the enclosing fragment.
    fn enclosing_of(
        ctx: &dartr_element::Ctx<'_>,
        element: ElementId,
    ) -> Option<(FragmentId, Option<ElementId>)> {
        let data = ctx.element_data(element)?;
        let enclosing = ctx.fragment_data(data.first_fragment)?.enclosing_fragment?;
        let enclosing_element = ctx
            .fragment_data(enclosing)
            .and_then(|f| f.element.try_get().copied());
        Some((enclosing, enclosing_element))
    }

    /// Dart `_inFactoryConstructor(element)`.
    fn in_factory_constructor(ctx: &dartr_element::Ctx<'_>, element: Option<ElementId>) -> bool {
        let mut element = element;
        // The recursion of Dart, as a loop.
        while let Some(e) = element {
            let Some((_, enclosing_element)) = Self::enclosing_of(ctx, e) else {
                return false;
            };
            if e.tag() == Tag::Constructor {
                return crate::element_ext::first_fragment_flags(ctx, e)
                    .contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY);
            }
            element = enclosing_element;
        }
        false
    }

    /// Dart `_inStaticMethod(element)`.
    fn in_static_method(ctx: &dartr_element::Ctx<'_>, element: Option<ElementId>) -> bool {
        let mut element = element;
        while let Some(e) = element {
            let Some((enclosing, enclosing_element)) = Self::enclosing_of(ctx, e) else {
                return false;
            };
            if (enclosing.is::<InterfaceFragment>() || enclosing.tag() == Tag::Extension)
                && e.is::<ExecutableElement>()
            {
                return dartr_typesystem::member::is_static(ctx, ElemRef::Base(e));
            }
            element = enclosing_element;
        }
        false
    }
}

/// Dart `displayName` of an executable element: `ConstructorElementImpl`
/// (`Class.name` or `Class`), `MethodElementImpl` (`lookupName`), else
/// `ElementImpl.displayName` (`name ?? '<unnamed>'`).
pub(crate) fn executable_display_name(ctx: &dartr_element::Ctx<'_>, e: ElementId) -> String {
    let name = ctx
        .element_data(e)
        .and_then(|d| d.name)
        .map(|n| ctx.name_str(n).to_string());
    match e.tag() {
        Tag::Constructor => {
            let class_name = ctx
                .element_data(e)
                .and_then(|d| d.enclosing)
                .and_then(|c| ctx.element_data(c))
                .and_then(|d| d.name)
                .map(|n| ctx.name_str(n).to_string())
                .unwrap_or_else(|| "<null>".to_string());
            let name = name.unwrap_or_else(|| "<null>".to_string());
            if name != "new" {
                format!("{class_name}.{name}")
            } else {
                class_name
            }
        }
        Tag::Method => dartr_typesystem::member::lookup_name(ctx, ElemRef::Base(e))
            .unwrap_or_else(|| "<unnamed>".to_string()),
        _ => name.unwrap_or_else(|| "<unnamed>".to_string()),
    }
}

use dartr_diagnostics::diag;
use dartr_element::diagnostics::type_arg;
use dartr_element::{
    EId, ElementFlags, FormalParameterElement, InstanceElement, TypeKind, TypeParameterElement,
    Variance,
};
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_typesystem::TypeExt;
use dartr_typesystem::member;

use super::{ErrorVerifier, ThisContext};
use crate::ast_ext::{self, formal_parameter_parts};
use crate::error::{return_type_verifier, type_arguments_verifier};

/// Dart `Variance.combine(other)`.
fn variance_combine(this: Variance, other: Variance) -> Variance {
    if this == Variance::Unrelated || other == Variance::Unrelated {
        return Variance::Unrelated;
    }
    if this == Variance::Invariant || other == Variance::Invariant {
        return Variance::Invariant;
    }
    if this == other {
        Variance::Covariant
    } else {
        Variance::Contravariant
    }
}

/// Dart `Variance.meet(other)` (the bitwise or of the encodings:
/// unrelated 0, covariant 1, contravariant 2, invariant 3).
fn variance_meet(this: Variance, other: Variance) -> Variance {
    let encode = |v: Variance| match v {
        Variance::Unrelated => 0,
        Variance::Covariant => 1,
        Variance::Contravariant => 2,
        Variance::Invariant => 3,
    };
    match encode(this) | encode(other) {
        0 => Variance::Unrelated,
        1 => Variance::Covariant,
        2 => Variance::Contravariant,
        _ => Variance::Invariant,
    }
}

/// Dart `TypeParameterElementImpl.computeVarianceInType(type)`: the
/// variance of [type_parameter] in [ty].
pub(crate) fn compute_variance_in_type(
    ctx: &dartr_element::Ctx<'_>,
    type_parameter: EId<TypeParameterElement>,
    ty: TypeId,
) -> Variance {
    match *ctx.ty(ty) {
        TypeKind::TypeParameter { param, .. } => {
            if param == type_parameter {
                Variance::Covariant
            } else {
                Variance::Unrelated
            }
        }
        TypeKind::Interface { element, args, .. } => {
            let mut result = Variance::Unrelated;
            let parameters = &ctx.instance(element.upcast()).type_params;
            for (i, &argument) in ctx.list(args).iter().enumerate() {
                let Some(&parameter) = parameters.get(i) else {
                    break;
                };
                let parameter_variance = ctx.type_parameter_variance(parameter);
                result = variance_meet(
                    result,
                    variance_combine(
                        parameter_variance,
                        compute_variance_in_type(ctx, type_parameter, argument),
                    ),
                );
            }
            result
        }
        TypeKind::Function(f) => {
            let mut result = compute_variance_in_type(ctx, type_parameter, f.ret);
            for &parameter in ctx.list(f.type_params) {
                // If [parameter] is referenced in the bound at all, it makes
                // the variance of [parameter] in the entire type invariant.
                if let Some(bound) = ctx.get(parameter).bound.get()
                    && compute_variance_in_type(ctx, type_parameter, bound) != Variance::Unrelated
                {
                    result = Variance::Invariant;
                }
            }
            for formal_parameter in ctx.list(f.params) {
                result = variance_meet(
                    result,
                    variance_combine(
                        Variance::Contravariant,
                        compute_variance_in_type(ctx, type_parameter, formal_parameter.ty),
                    ),
                );
            }
            result
        }
        _ => Variance::Unrelated,
    }
}

/// Dart `TypeParameterizedElement.isSimplyBounded` of [element].
fn is_simply_bounded(ctx: &dartr_element::Ctx<'_>, element: ElementId) -> bool {
    let Some(data) = ctx.element_data(element) else {
        return true;
    };
    if element.is::<InstanceElement>() {
        data.flags
            .has(ElementFlags::INSTANCE_ELEMENT_IS_SIMPLY_BOUNDED)
    } else if element.tag() == Tag::TypeAlias {
        data.flags
            .has(ElementFlags::TYPE_ALIAS_ELEMENT_IS_SIMPLY_BOUNDED)
    } else {
        true
    }
}

/// Dart `SuperFormalParameterElementImpl.superConstructorParameter` (the
/// base element).
fn super_constructor_parameter(
    ctx: &dartr_element::Ctx<'_>,
    p: EId<FormalParameterElement>,
) -> Option<EId<FormalParameterElement>> {
    let constructor = ctx
        .element_data(p.raw())?
        .enclosing?
        .cast::<dartr_element::ConstructorElement>()?;
    let ElemRef::Base(super_constructor) = ctx.get(constructor).super_constructor.get()? else {
        return None;
    };
    let super_constructor = super_constructor.cast::<dartr_element::ConstructorElement>()?;
    let super_params = &ctx.get(super_constructor).formal_params;
    let pe = ctx.get(p);
    if pe.kind.is_named() {
        super_params
            .iter()
            .copied()
            .find(|&s| ctx.get(s).kind.is_named() && ctx.get(s).name == pe.name)
    } else {
        let index = ctx
            .get(constructor)
            .formal_params
            .iter()
            .copied()
            .filter(|x| {
                x.raw().tag() == Tag::SuperFormalParameter && ctx.get(*x).kind.is_positional()
            })
            .position(|x| x == p)?;
        super_params
            .iter()
            .copied()
            .filter(|&s| ctx.get(s).kind.is_positional())
            .nth(index)
    }
}

/// Whether a fragment of the formal parameter [p] has a default value
/// (Dart `constantInitializer2 != null`).
fn has_constant_initializer(ctx: &dartr_element::Ctx<'_>, p: EId<FormalParameterElement>) -> bool {
    if let Some(Some(_)) = ctx.get(p).constant_initializer.try_get() {
        return true;
    }
    let mut fragment = ctx.element_data(p.raw()).map(|d| d.first_fragment);
    while let Some(f) = fragment {
        if let Some(id) = f.cast::<dartr_element::VariableFragment>()
            && ctx
                .store(f.store())
                .variable_fragment(id)
                .constant_initializer
                .is_some()
        {
            return true;
        }
        fragment = ctx.fragment_data(f).and_then(|d| d.next_fragment);
    }
    false
}

/// Dart `FormalParameterElement.hasDefaultValue` (`defaultValueCode !=
/// null`). For a super formal parameter without a default value, Dart
/// uses the default value of the super constructor parameter when its
/// constant value is a subtype of the type of [p]; without constant
/// evaluation, it is assumed to be when the super parameter has a valid
/// type.
fn has_default_value(
    ctx: &dartr_element::Ctx<'_>,
    p: EId<FormalParameterElement>,
    has_default_clause: bool,
) -> bool {
    let is_super = p.raw().tag() == Tag::SuperFormalParameter;
    if is_super && ctx.get(p).kind.is_required() {
        return false;
    }
    if has_default_clause || has_constant_initializer(ctx, p) {
        return true;
    }
    if is_super {
        let mut current = p;
        // A chain of super parameters, with a limit for cycles.
        for _ in 0..32 {
            let Some(super_parameter) = super_constructor_parameter(ctx, current) else {
                return false;
            };
            if ctx
                .get(super_parameter)
                .type_
                .get()
                .is_none_or(|t| t == TypeId::INVALID)
            {
                return false;
            }
            if has_constant_initializer(ctx, super_parameter) {
                return true;
            }
            if super_parameter.raw().tag() != Tag::SuperFormalParameter
                || ctx.get(super_parameter).kind.is_required()
            {
                return false;
            }
            current = super_parameter;
        }
    }
    false
}

impl ErrorVerifier<'_> {
    /// Dart `_featureSet.isEnabled(feature)` (the library feature set).
    fn is_feature_enabled(&self, flag: ExperimentalFlag) -> bool {
        self.ctx
            .get(self.unit.library)
            .feature_set
            .is_enabled(flag.name())
    }

    /// Dart `token.lexeme`.
    fn lexeme(&self, token: TokenId) -> &str {
        self.ast.tokens.lexeme(token)
    }

    /// Dart `typeAnnotation.typeOrThrow` (`None` when not resolved).
    fn annotation_type(&self, node: impl Into<dartr_ast::NodeId>) -> Option<TypeId> {
        self.tables.annotation_type.get(node).copied()
    }

    /// Dart `node.declaredFragment!.element`.
    pub(super) fn declared_element(&self, node: impl Into<dartr_ast::NodeId>) -> Option<ElementId> {
        let fragment = *self.tables.declared_fragment.get(node)?;
        self.ctx.fragment_data(fragment)?.element.try_get().copied()
    }

    /// Dart `visitFormalParameterList`.
    pub(super) fn visit_formal_parameter_list(&mut self, node: Id<FormalParameterList>) {
        self.with_duplicate_definition_verifier(|d, this| d.check_parameters(this, node));
        self.check_use_of_covariant_in_parameters(node);
        self.check_use_of_default_values_in_parameters(node);
        self.visit_children(node);
    }

    /// Dart `visitFunctionDeclaration`.
    pub(super) fn visit_function_declaration(&mut self, node: Id<FunctionDeclaration>) {
        let Some(&fragment) = self.tables.declared_fragment.get(node) else {
            self.visit_children(node);
            return;
        };
        let element = self
            .ctx
            .fragment_data(fragment)
            .and_then(|f| f.element.try_get().copied());
        let n = &self.ast[node];
        let (name, augment_keyword, return_type, external_keyword, property_keyword) = (
            n.name,
            n.augment_keyword,
            n.return_type,
            n.external_keyword,
            n.property_keyword,
        );
        let function_expression = n.function_expression;
        let fe = &self.ast[function_expression];
        let (type_parameters, parameters, body) = (fe.type_parameters, fe.parameters, fe.body);

        let has_const_variable_augmentation =
            self.check_for_const_variable_augmentation_by_accessor(name, fragment);
        if !has_const_variable_augmentation {
            self.check_augmentation_without_declaration(augment_keyword, fragment);
            self.check_for_function_already_complete(augment_keyword, fragment);
        }
        self.check_for_function_body_completeness(node.raw(), name, fragment, body);
        let first_type_parameters = self.first_fragment_type_parameters(element);
        self.check_for_augmentation_type_parameters(
            fragment,
            &first_type_parameters,
            name,
            type_parameters,
        );
        self.check_for_augmentation_return_type_mismatch(
            fragment,
            return_type,
            match return_type {
                Some(t) => Entity::Node(t.raw()),
                None => Entity::Token(name),
            },
        );
        if let Some(parameters) = parameters {
            self.check_for_augmentation_formal_parameters(fragment, parameters);
        }

        if let Some(element) = element {
            // Dart `element.enclosingElement is! LibraryElement`. The
            // element data of a local function has no enclosing element, so
            // test the kind: only a top-level function is in the library.
            let enclosing_is_library = element.tag() == Tag::TopLevelFunction;
            if !enclosing_is_library && let Some(hidden) = self.hidden_elements.as_mut() {
                hidden.declare(element);
            }
        }

        let is_asynchronous = ast_ext::function_body_is_asynchronous(self.ast, body.raw());
        let is_generator = ast_ext::function_body_is_generator(self.ast, body.raw());
        let context =
            EnclosingExecutableContext::new(&self.ctx, element, is_asynchronous, is_generator);
        let is_setter = ast_ext::is_keyword(self.ast, property_keyword, "set");
        self.with_enclosing_executable(context, |this| {
            if is_setter {
                this.check_for_non_void_return_type_for_setter(return_type);
            }
            this.check_for_type_annotation_deferred_class(return_type);
            this.with_return_type_context(|this, context| {
                return_type_verifier::verify_return_type(this, context, return_type)
            });
            this.check_for_main_function1(name, fragment);
            this.check_for_main_function2(node);
            this.check_for_external_method_with_body(external_keyword, body);
            this.visit_children(node);
        });
    }

    /// Dart `element.firstFragment.typeParameters` of an executable.
    fn first_fragment_type_parameters(&self, element: Option<ElementId>) -> Vec<FragmentId> {
        let Some(element) = element else {
            return Vec::new();
        };
        let Some(e) = element.cast::<ExecutableElement>() else {
            return Vec::new();
        };
        self.ctx
            .executable(e)
            .type_params
            .iter()
            .filter_map(|&tp| {
                let data = self.ctx.element_data(tp.raw())?;
                Some(data.first_fragment)
            })
            .collect()
    }

    /// Dart `visitFunctionExpression`.
    pub(super) fn visit_function_expression(&mut self, node: Id<FunctionExpression>) {
        self.is_in_late_local_variable.push(false);

        let parent_is_declaration = self
            .ast
            .parent(node)
            .is_some_and(|p| self.ast.is::<FunctionDeclaration>(p));
        if parent_is_declaration {
            self.visit_children(node);
        } else {
            let element = self.declared_element(node);
            let body = self.ast[node].body;
            let context = EnclosingExecutableContext::new(
                &self.ctx,
                element,
                ast_ext::function_body_is_asynchronous(self.ast, body.raw()),
                ast_ext::function_body_is_generator(self.ast, body.raw()),
            );
            self.with_enclosing_executable(context, |this| {
                this.visit_children(node);
            });
        }

        self.is_in_late_local_variable.pop();
    }

    /// Dart `visitMethodDeclaration`.
    pub(super) fn visit_method_declaration(&mut self, node: Id<MethodDeclaration>) {
        let Some(&fragment) = self.tables.declared_fragment.get(node) else {
            self.visit_children(node);
            return;
        };
        let element = self
            .ctx
            .fragment_data(fragment)
            .and_then(|f| f.element.try_get().copied());
        let n = &self.ast[node];
        let name = n.name;
        let augment_keyword = n.augment_keyword;
        let return_type = n.return_type;
        let type_parameters = n.type_parameters;
        let parameters = n.parameters;
        let external_keyword = n.external_keyword;
        let body = n.body;
        let is_setter = ast_ext::is_keyword(self.ast, n.property_keyword, "set");
        let is_operator = n.operator_keyword.is_some();
        let is_static = ast_ext::is_keyword(self.ast, n.modifier_keyword, "static");

        let has_const_variable_augmentation =
            self.check_for_const_variable_augmentation_by_accessor(name, fragment);
        if !has_const_variable_augmentation {
            self.check_augmentation_without_declaration(augment_keyword, fragment);
            self.check_for_function_already_complete(augment_keyword, fragment);
        }
        self.check_for_function_body_completeness(node.raw(), name, fragment, body);
        self.check_for_extension_declares_abstract_member(node);
        let first_type_parameters = self.first_fragment_type_parameters(element);
        self.check_for_augmentation_type_parameters(
            fragment,
            &first_type_parameters,
            name,
            type_parameters,
        );
        self.check_for_augmentation_return_type_mismatch(
            fragment,
            return_type,
            match return_type {
                Some(t) => Entity::Node(t.raw()),
                None => Entity::Token(name),
            },
        );
        if let Some(parameters) = parameters {
            self.check_for_augmentation_formal_parameters(fragment, parameters);
        }

        let context = EnclosingExecutableContext::new(
            &self.ctx,
            element,
            ast_ext::function_body_is_asynchronous(self.ast, body.raw()),
            ast_ext::function_body_is_generator(self.ast, body.raw()),
        );
        self.with_enclosing_executable(context, |this| {
            if is_setter {
                this.check_for_non_void_return_type_for_setter(return_type);
            } else if is_operator {
                let has_wrong_number_of_parameters =
                    this.check_for_wrong_number_of_parameters_for_operator(node);
                if !has_wrong_number_of_parameters {
                    // If the operator has too many parameters including one
                    // or more optional parameters, only report one error.
                    this.check_for_optional_parameter_in_operator(node);
                }
                this.check_for_non_void_return_type_for_operator(node);
            }
            this.check_for_extension_declares_member_of_object(node);
            this.check_for_type_annotation_deferred_class(return_type);
            this.with_return_type_context(|this, context| {
                return_type_verifier::verify_return_type(this, context, return_type)
            });
            this.check_for_wrong_type_parameter_variance_in_method(node);
            this.check_for_external_method_with_body(external_keyword, body);

            // Dart `node.visitChildrenWithHooks(this, visitBody: ...)`.
            for child in this.ast.children(node) {
                if child == body.raw() {
                    let state = if is_static {
                        ThisContext::StaticMemberBody
                    } else {
                        ThisContext::InstanceMemberBody
                    };
                    this.with_this_context(state, |this| this.accept(child));
                } else {
                    this.accept(child);
                }
            }
        });
    }

    /// Dart `visitNamedType`.
    pub(super) fn visit_named_type(&mut self, node: Id<NamedType>) {
        let name = self.ast[node].name;
        let element = self.element(node);
        self.check_for_ambiguous_import(name, element);
        self.check_for_type_parameter_referenced_by_static(name, element);
        type_arguments_verifier::check_named_type(self, node);
        self.visit_children(node);
    }

    /// Dart `visitRegularFormalParameter`.
    pub(super) fn visit_regular_formal_parameter(&mut self, node: Id<RegularFormalParameter>) {
        let parameter: Id<FormalParameter> = Id::from_raw(node.raw());
        self.check_for_default_value_assignable_at_type(parameter);
        let type_ = self.ast[node].type_;
        self.check_for_type_annotation_deferred_class(type_);
        self.check_private_optional_parameter(parameter);
        self.visit_children(node);
    }

    /// Dart `visitTypeArgumentList`.
    pub(super) fn visit_type_argument_list(&mut self, node: Id<TypeArgumentList>) {
        let list = self.ast[node].arguments;
        for &type_ in self.ast.list(list) {
            self.check_for_type_annotation_deferred_class(Some(type_));
        }
        self.visit_children(node);
    }

    /// Dart `visitTypeParameter`.
    pub(super) fn visit_type_parameter(&mut self, node: Id<TypeParameter>) {
        let n = &self.ast[node];
        let (name, bound) = (n.name, n.bound);
        self.check_for_built_in_identifier_as_name(
            name,
            diag::built_in_identifier_as_type_parameter_name,
        );
        self.check_for_type_annotation_deferred_class(bound);
        self.check_for_generic_function_type(bound);
        if let Some(bound) = bound {
            self.uninstantiated_bound_check(bound.raw());
        }
        self.visit_children(node);
    }

    /// Dart `visitTypeParameterList`.
    pub(super) fn visit_type_parameter_list(&mut self, node: Id<TypeParameterList>) {
        self.with_duplicate_definition_verifier(|d, this| d.check_type_parameters(this, node));
        let parameters = self.ast.list(self.ast[node].type_parameters).to_vec();
        self.check_for_type_parameter_bound_recursion(&parameters);
        self.visit_children(node);
    }

    /// Dart `_UninstantiatedBoundChecker` (a `RecursiveAstVisitor` with a
    /// `visitNamedType` override) on [node].
    fn uninstantiated_bound_check(&mut self, node: dartr_ast::NodeId) {
        if let Some(named_type) = self.ast.cast::<NamedType>(node) {
            if let Some(type_args) = self.ast[named_type].type_arguments {
                self.uninstantiated_bound_check(type_args.raw());
                return;
            }
            if let Some(element) = self.element(named_type) {
                let element = member::base_element(&self.ctx, element);
                if !is_simply_bounded(&self.ctx, element) {
                    // TODO(srawlins): Don't report this if
                    //  TYPE_ALIAS_CANNOT_REFERENCE_ITSELF has been reported.
                    self.report_at(diag::not_instantiated_bound(), named_type);
                }
            }
            return;
        }
        for child in self.ast.children(node) {
            self.uninstantiated_bound_check(child);
        }
    }

    /// Dart `_checkForConstVariableAugmentationByAccessor(errorToken:,
    /// fragment:)`. Augmentations: not ported (the experiment is off).
    fn check_for_const_variable_augmentation_by_accessor(
        &mut self,
        error_token: TokenId,
        fragment: FragmentId,
    ) -> bool {
        let _ = (error_token, fragment);
        false
    }

    /// Dart `_checkForFunctionAlreadyComplete(augmentKeyword:, fragment:)`.
    /// Augmentations: not ported (the experiment is off).
    fn check_for_function_already_complete(
        &mut self,
        augment_keyword: Option<TokenId>,
        fragment: FragmentId,
    ) {
        let _ = (augment_keyword, fragment);
    }

    /// Dart `_checkForDefaultValueAlreadySpecifiedInAugmentationChain(
    /// parameter)`. Augmentations: not ported (the experiment is off).
    fn check_for_default_value_already_specified_in_augmentation_chain(
        &mut self,
        parameter: Id<FormalParameter>,
    ) {
        let _ = parameter;
    }

    /// Dart `_checkForExtensionDeclaresAbstractMember(node)`.
    fn check_for_extension_declares_abstract_member(&mut self, node: Id<MethodDeclaration>) {
        if self.is_feature_enabled(ExperimentalFlag::Augmentations) {
            return;
        }

        if self.enclosing_extension.is_none() {
            return;
        }

        let n = &self.ast[node];
        // Static members without bodies are already reported by the parser.
        if ast_ext::is_keyword(self.ast, n.modifier_keyword, "static") {
            return;
        }

        let is_complete =
            n.external_keyword.is_some() || !self.ast.is::<EmptyFunctionBody>(n.body.raw());
        if !is_complete {
            let name = n.name;
            self.report_at_token(diag::extension_declares_abstract_member(), name);
        }
    }

    /// Dart `_checkForExtensionDeclaresMemberOfObject(node)`.
    fn check_for_extension_declares_member_of_object(&mut self, node: Id<MethodDeclaration>) {
        let name = self.ast[node].name;
        let lexeme = self.lexeme(name);
        if self.enclosing_extension.is_some() && self.ctx.tp.is_object_member(&self.ctx, lexeme) {
            self.report_at_token(diag::extension_declares_member_of_object(), name);
        }

        if self
            .enclosing_class
            .is_some_and(|c| c.tag() == Tag::ExtensionType)
            && self.ctx.tp.is_object_member(&self.ctx, self.lexeme(name))
        {
            self.report_at_token(diag::extension_type_declares_member_of_object(), name);
        }
    }

    /// Dart `_checkForFunctionBodyCompleteness(node:, nameToken:,
    /// fragment:)`; [body] is the body of [node].
    fn check_for_function_body_completeness(
        &mut self,
        node: dartr_ast::NodeId,
        name_token: TokenId,
        fragment: FragmentId,
        body: Id<FunctionBody>,
    ) {
        if !self.is_feature_enabled(ExperimentalFlag::Augmentations) {
            return;
        }

        let Some(fragment_data) = self.ctx.fragment_data(fragment) else {
            return;
        };
        // Report only on the introductory declaration.
        if fragment_data
            .flags
            .has(FragmentFlags::FRAGMENT_IS_AUGMENTATION)
        {
            return;
        }

        let Some(&element) = fragment_data.element.try_get() else {
            return;
        };
        let Some(element_data) = self.ctx.element_data(element) else {
            return;
        };
        let enclosing_element = element_data.enclosing;
        let is_static = member::is_static(&self.ctx, ElemRef::Base(element));

        // Instance members are validated for the whole interface.
        if !is_static
            && enclosing_element
                .is_some_and(|e| matches!(e.tag(), Tag::Class | Tag::Enum | Tag::Mixin))
        {
            return;
        }

        let Some(name) = element_data.name else {
            return;
        };
        let name = self.ctx.name_str(name).to_string();

        let mut fragments = Vec::new();
        let mut f = Some(element_data.first_fragment);
        while let Some(id) = f {
            let Some(data) = self.ctx.fragment_data(id) else {
                break;
            };
            fragments.push(data.flags.has(FragmentFlags::FRAGMENT_IS_COMPLETE));
            f = data.next_fragment;
        }
        if fragments.iter().any(|&complete| complete) {
            return;
        }

        if fragments.len() == 1 {
            match enclosing_element.map(|e| e.tag()) {
                Some(Tag::Extension) if !is_static => {
                    self.report_at_token(diag::extension_declares_abstract_member(), name_token);
                }
                Some(Tag::ExtensionType) if !is_static => {
                    let extension_type_name = enclosing_element
                        .and_then(|e| self.ctx.element_data(e))
                        .and_then(|d| d.name)
                        .map(|n| self.ctx.name_str(n).to_string())
                        .unwrap_or_default();
                    self.report_at(
                        diag::extension_type_with_abstract_member(&name, &extension_type_name),
                        node,
                    );
                }
                _ => {
                    if let Some(empty) = self.ast.cast::<EmptyFunctionBody>(body.raw()) {
                        let error_token = self.ast[empty].semicolon;
                        self.report_at_token(diag::missing_function_body(), error_token);
                    }
                }
            }
        } else {
            self.report_at_token(
                diag::function_not_complete_after_augmentations(&name),
                name_token,
            );
        }
    }

    /// Dart `_checkForGenericFunctionType(node)`.
    fn check_for_generic_function_type(&mut self, node: Option<Id<TypeAnnotation>>) {
        let Some(node) = node else {
            return;
        };
        if self.is_feature_enabled(ExperimentalFlag::GenericMetadata) {
            return;
        }
        let Some(ty) = self.annotation_type(node) else {
            return;
        };
        if let TypeKind::Function(f) = *self.ctx.ty(ty)
            && !self.ctx.list(f.type_params).is_empty()
        {
            self.report_at(diag::generic_function_type_cannot_be_bound(), node);
        }
    }

    /// Dart `_checkForMainFunction2(functionDeclaration)`.
    fn check_for_main_function2(&mut self, function_declaration: Id<FunctionDeclaration>) {
        let n = &self.ast[function_declaration];
        let name = n.name;
        if self.lexeme(name) != "main" {
            return;
        }

        if !self
            .ast
            .parent(function_declaration)
            .is_some_and(|p| self.ast.is::<CompilationUnit>(p))
        {
            return;
        }

        let Some(parameter_list) = self.ast[n.function_expression].parameters else {
            return;
        };

        let parameters = self.ast.list(self.ast[parameter_list].parameters).to_vec();
        let kinds: Vec<_> = parameters
            .iter()
            .map(|&p| formal_parameter_parts(self.ast, p.raw()))
            .collect();
        let positional: Vec<usize> = (0..parameters.len())
            .filter(|&i| kinds[i].kind.is_positional())
            .collect();
        let required_positional = kinds
            .iter()
            .filter(|k| k.kind.is_required_positional())
            .count();

        if required_positional > 2 {
            self.report_at_token(
                diag::main_has_too_many_required_positional_parameters(),
                name,
            );
        }

        if kinds.iter().any(|k| k.kind.is_required_named()) {
            self.report_at_token(diag::main_has_required_named_parameters(), name);
        }

        if let Some(&first) = positional.first() {
            let Some(element) = self.declared_element(parameters[first]) else {
                return;
            };
            let ty = member::type_(&self.ctx, ElemRef::Base(element));
            let list_of_string = self.ctx.tp.list_type(&self.ctx, self.ctx.tp.string_type());
            if !self.type_system.is_subtype_of(list_of_string, ty) {
                let target = match kinds[first].type_ {
                    Some(t) => t.raw(),
                    None => parameters[first].raw(),
                };
                self.report_at(diag::main_first_positional_parameter_type(), target);
            }
        }
    }

    /// Dart `_checkForNonVoidReturnTypeForOperator(declaration)`.
    fn check_for_non_void_return_type_for_operator(&mut self, declaration: Id<MethodDeclaration>) {
        // check that []= operator
        if self.lexeme(self.ast[declaration].name) != "[]=" {
            return;
        }
        // check return type
        if let Some(annotation) = self.ast[declaration].return_type
            && let Some(ty) = self.annotation_type(annotation)
            && *self.ctx.ty(ty) != TypeKind::Void
        {
            self.report_at(diag::non_void_return_for_operator(), annotation);
        }
    }

    /// Dart `_checkForNonVoidReturnTypeForSetter(namedType)`.
    fn check_for_non_void_return_type_for_setter(
        &mut self,
        named_type: Option<Id<TypeAnnotation>>,
    ) {
        if let Some(named_type) = named_type
            && let Some(ty) = self.annotation_type(named_type)
            && *self.ctx.ty(ty) != TypeKind::Void
        {
            self.report_at(diag::non_void_return_for_setter(), named_type);
        }
    }

    /// Dart `_checkForOptionalParameterInOperator(declaration)`.
    fn check_for_optional_parameter_in_operator(&mut self, declaration: Id<MethodDeclaration>) {
        let Some(parameter_list) = self.ast[declaration].parameters else {
            return;
        };
        let formal_parameters = self.ast.list(self.ast[parameter_list].parameters).to_vec();
        for formal_parameter in formal_parameters {
            let parts = formal_parameter_parts(self.ast, formal_parameter.raw());
            if !parts.kind.is_required_positional() {
                self.report_at(diag::optional_parameter_in_operator(), formal_parameter);
            }
        }
    }

    /// Dart `_checkForWrongNumberOfParametersForOperator(declaration)`.
    fn check_for_wrong_number_of_parameters_for_operator(
        &mut self,
        declaration: Id<MethodDeclaration>,
    ) -> bool {
        // prepare number of parameters
        let Some(parameter_list) = self.ast[declaration].parameters else {
            return false;
        };
        let num_parameters = self.ast.list(self.ast[parameter_list].parameters).len() as i64;
        // prepare operator name
        let name_token = self.ast[declaration].name;
        let name = self.lexeme(name_token).to_string();
        // check for exact number of parameters
        let expected: i64 = match name.as_str() {
            "[]=" => 2,
            "<" | ">" | "<=" | ">=" | "==" | "+" | "/" | "~/" | "*" | "%" | "|" | "^" | "&"
            | "<<" | ">>" | ">>>" | "[]" => 1,
            "~" => 0,
            _ => -1,
        };
        if expected != -1 && num_parameters != expected {
            self.report_at_token(
                diag::wrong_number_of_parameters_for_operator(&name, expected, num_parameters),
                name_token,
            );
            return true;
        } else if name == "-" && num_parameters > 1 {
            self.report_at_token(
                diag::wrong_number_of_parameters_for_operator_minus(num_parameters),
                name_token,
            );
            return true;
        }
        false
    }

    /// Dart `_checkForWrongTypeParameterVarianceInMethod(method)`.
    fn check_for_wrong_type_parameter_variance_in_method(&mut self, method: Id<MethodDeclaration>) {
        // Only need to report errors for parameters with explicitly defined
        // type parameters in classes or mixins.
        let Some(enclosing_class) = self.enclosing_class else {
            return;
        };

        let type_parameters = self
            .ctx
            .instance(enclosing_class.upcast())
            .type_params
            .clone();
        let m = &self.ast[method];
        let (method_type_parameters, method_parameters, return_type) =
            (m.type_parameters, m.parameters, m.return_type);
        for type_parameter in type_parameters {
            if self.ctx.type_parameter_is_legacy_covariant(type_parameter) {
                continue;
            }

            if let Some(list) = method_type_parameters {
                for &method_type_parameter in self.ast.list(self.ast[list].type_parameters) {
                    let Some(bound) = self.ast[method_type_parameter].bound else {
                        continue;
                    };
                    let bound_type = self.annotation_type(bound).unwrap_or(TypeId::INVALID);
                    let method_type_parameter_variance = variance_combine(
                        Variance::Invariant,
                        compute_variance_in_type(&self.ctx, type_parameter, bound_type),
                    );
                    self.check_for_wrong_variance_position(
                        method_type_parameter_variance,
                        type_parameter.raw(),
                        Entity::Node(method_type_parameter.raw()),
                    );
                }
            }

            if let Some(list) = method_parameters {
                let parameters = self.ast.list(self.ast[list].parameters).to_vec();
                for method_parameter in parameters {
                    let Some(element) = self.declared_element(method_parameter) else {
                        continue;
                    };
                    if member::is_covariant(&self.ctx, ElemRef::Base(element)) {
                        continue;
                    }
                    let ty = member::type_(&self.ctx, ElemRef::Base(element));
                    let method_parameter_variance = variance_combine(
                        Variance::Contravariant,
                        compute_variance_in_type(&self.ctx, type_parameter, ty),
                    );
                    self.check_for_wrong_variance_position(
                        method_parameter_variance,
                        type_parameter.raw(),
                        Entity::Node(method_parameter.raw()),
                    );
                }
            }

            if let Some(return_type) = return_type {
                let ty = self.annotation_type(return_type).unwrap_or(TypeId::INVALID);
                let method_return_type_variance =
                    compute_variance_in_type(&self.ctx, type_parameter, ty);
                self.check_for_wrong_variance_position(
                    method_return_type_variance,
                    type_parameter.raw(),
                    Entity::Node(return_type.raw()),
                );
            }
        }
    }

    /// Dart `_checkUseOfCovariantInParameters(node)`.
    fn check_use_of_covariant_in_parameters(&mut self, node: Id<FormalParameterList>) {
        let parent = self.ast.parent(node);
        let parent_is = |kind: NodeKind| parent.is_some_and(|p| self.ast.kind(p) == kind);
        if self.enclosing_class.is_some() && parent_is(NodeKind::MethodDeclaration) {
            // Either [parent] is a static method, in which case
            // `EXTRANEOUS_MODIFIER` is reported by the parser, or [parent] is
            // an instance method, in which case any use of `covariant` is
            // legal.
            return;
        }

        // Parser reports `invalidCovariantModifierInPrimaryConstructor`.
        if parent_is(NodeKind::PrimaryConstructorDeclaration) {
            return;
        }

        if self.enclosing_extension.is_some() {
            // `INVALID_USE_OF_COVARIANT_IN_EXTENSION` is reported by the
            // parser.
            return;
        }

        if let Some(parent) = parent
            && self.ast.is::<FunctionExpression>(parent)
            && let Some(parent2) = self.ast.parent(parent)
            && self.ast.is::<FunctionDeclaration>(parent2)
            && self
                .ast
                .parent(parent2)
                .is_some_and(|p| self.ast.is::<CompilationUnit>(p))
        {
            // `EXTRANEOUS_MODIFIER` is reported by the parser, for
            // library-level functions.
            return;
        }

        let parameters = self.ast.list(self.ast[node].parameters).to_vec();
        for parameter in parameters {
            let parts = formal_parameter_parts(self.ast, parameter.raw());
            if let Some(keyword) = parts.covariant_keyword {
                self.report_at_token(diag::invalid_use_of_covariant(), keyword);
            }
        }
    }

    /// Dart `_checkUseOfDefaultValuesInParameters(node)`.
    fn check_use_of_default_values_in_parameters(&mut self, node: Id<FormalParameterList>) {
        let default_values_are_expected = (|| {
            let Some(parent) = self.ast.parent(node) else {
                return false;
            };
            if let Some(parent) = self.ast.cast::<ConstructorDeclaration>(parent) {
                let p = &self.ast[parent];
                if p.external_keyword.is_some()
                    || (p.factory_keyword.is_some() && p.redirected_constructor.is_some())
                {
                    return false;
                }
                true
            } else if let Some(parent) = self.ast.cast::<FunctionExpression>(parent) {
                if let Some(parent2) = self.ast.parent(parent)
                    && let Some(parent2) = self.ast.cast::<FunctionDeclaration>(parent2)
                    && self.ast[parent2].external_keyword.is_some()
                {
                    return false;
                }
                !self
                    .ast
                    .is::<NativeFunctionBody>(self.ast[parent].body.raw())
            } else if let Some(parent) = self.ast.cast::<MethodDeclaration>(parent) {
                let p = &self.ast[parent];
                let is_complete =
                    p.external_keyword.is_some() || !self.ast.is::<EmptyFunctionBody>(p.body.raw());
                if !is_complete || p.external_keyword.is_some() {
                    return false;
                }
                !self.ast.is::<NativeFunctionBody>(p.body.raw())
            } else {
                self.ast.is::<PrimaryConstructorDeclaration>(parent)
            }
        })();

        let parameters = self.ast.list(self.ast[node].parameters).to_vec();
        for parameter in parameters {
            self.check_for_default_value_already_specified_in_augmentation_chain(parameter);

            let parts = formal_parameter_parts(self.ast, parameter.raw());
            if parts.kind.is_required_named() {
                if parts.default_clause.is_some() {
                    match parts.name {
                        Some(name) => {
                            self.report_at_token(diag::default_value_on_required_parameter(), name)
                        }
                        None => {
                            self.report_at(diag::default_value_on_required_parameter(), parameter)
                        }
                    }
                }
            } else if default_values_are_expected && parts.kind.is_optional() {
                let Some(element) = self.declared_element(parameter) else {
                    continue;
                };
                let Some(element) = element.cast::<FormalParameterElement>() else {
                    continue;
                };
                if has_default_value(&self.ctx, element, parts.default_clause.is_some()) {
                    continue;
                }
                let ty = member::type_(&self.ctx, ElemRef::Base(element.raw()));
                if !self.type_system.is_potentially_non_nullable(ty) {
                    continue;
                }
                let parameter_name = parts.name;
                let d = if self.metadata_has_required(parts.metadata) {
                    diag::missing_default_value_for_parameter_with_annotation()
                } else {
                    if self.is_wildcard_super_formal_positional_parameter(parameter) {
                        continue;
                    }
                    let name = parameter_name
                        .map(|t| self.lexeme(t).to_string())
                        .unwrap_or_else(|| "?".to_string());
                    if parts.kind.is_positional() {
                        diag::missing_default_value_for_parameter_positional(&name)
                    } else {
                        diag::missing_default_value_for_parameter(&name)
                    }
                };
                match parameter_name {
                    Some(name) => self.report_at_token(d, name),
                    None => self.report_at(d, parameter),
                }
            }
        }
    }

    /// Dart `parameterElement.metadata.hasRequired`: an annotation is the
    /// `required` getter or a `Required` constructor of `package:meta`.
    fn metadata_has_required(&self, metadata: NodeList<Annotation>) -> bool {
        self.ast.list(metadata).iter().any(|&annotation| {
            let Some(element) = self.element(annotation) else {
                return false;
            };
            let element = member::base_element(&self.ctx, element);
            let in_meta = self
                .ctx
                .element_library_uri(element)
                .is_some_and(|uri| uri == "package:meta/meta.dart");
            if !in_meta {
                return false;
            }
            match element.tag() {
                Tag::Getter => self.ctx.element_name(element) == Some("required"),
                Tag::Constructor => {
                    self.ctx
                        .element_data(element)
                        .and_then(|d| d.enclosing)
                        .and_then(|c| self.ctx.element_name(c))
                        == Some("Required")
                }
                _ => false,
            }
        })
    }

    /// Dart `_isWildcardSuperFormalPositionalParameter(parameter)`.
    fn is_wildcard_super_formal_positional_parameter(
        &self,
        parameter: Id<FormalParameter>,
    ) -> bool {
        let Some(p) = self.ast.cast::<SuperFormalParameter>(parameter) else {
            return false;
        };
        let p = &self.ast[p];
        p.kind.is_positional()
            && self.lexeme(p.name) == "_"
            && self.is_feature_enabled(ExperimentalFlag::WildcardVariables)
    }

    /// Dart `_getLibraryName(element)`: the URI of the library of
    /// [element], with the libraries that export it to this unit.
    pub(crate) fn get_library_name(&self, element: Option<ElementId>) -> String {
        let Some(element) = element else {
            return String::new();
        };
        let Some(library) = self.ctx.element_data(element).and_then(|d| d.library) else {
            return String::new();
        };
        let Some(name) = self.ctx.element_name(element) else {
            return String::new();
        };
        let library_uri = self.ctx.library_uri(library).to_string();
        let imports = self.unit_imports_with_enclosing();
        if imports
            .iter()
            .any(|&(imported, _)| imported == Some(library))
        {
            return library_uri;
        }
        let mut indirect_sources: Vec<String> = Vec::new();
        for &(imported, ref namespace) in &imports {
            if let Some(imported) = imported
                && let Some(namespace) = namespace
                && namespace.defined_names.get(&self.ctx.name(name)) == Some(&element)
            {
                indirect_sources.push(self.ctx.library_uri(imported).to_string());
            }
        }
        let mut buffer = library_uri;
        if !indirect_sources.is_empty() {
            buffer.push_str(" (via ");
            if indirect_sources.len() > 1 {
                indirect_sources.sort();
                buffer.push_str(&quoted_and_comma_separated_with_and(&indirect_sources));
            } else {
                buffer.push_str(&indirect_sources[0]);
            }
            buffer.push(')');
        }
        buffer
    }

    /// Dart `_currentUnit.withEnclosing.expand((f) => f.libraryImports)`:
    /// the imported library and the import namespace of each import (the
    /// export namespace of the imported library when the import namespace
    /// is not computed yet).
    fn unit_imports_with_enclosing(
        &self,
    ) -> Vec<(
        Option<dartr_element::EId<dartr_element::LibraryElement>>,
        Option<std::sync::Arc<dartr_element::Namespace>>,
    )> {
        let mut result = Vec::new();
        let mut current = Some(self.unit.fragment);
        while let Some(f) = current {
            for import in &self.ctx.fragment(f).library_imports {
                let imported = match &import.directive.uri {
                    dartr_element::DirectiveUri::Library { library, .. } => Some(*library),
                    _ => None,
                };
                let namespace = import.namespace.try_get().cloned().or_else(|| {
                    imported.and_then(|l| self.ctx.get(l).export_namespace.try_get().cloned())
                });
                result.push((imported, namespace));
            }
            current = self.unit.scopes.enclosing_fragment(f);
        }
        result
    }

    // Helpers that other sections call.

    /// Dart `_checkForAugmentationFormalParameters(executableFragment:,
    /// formalParameterList:)`. Augmentations: not ported (the experiment
    /// is off).
    pub(crate) fn check_for_augmentation_formal_parameters(
        &mut self,
        executable_fragment: FragmentId,
        formal_parameter_list: Id<FormalParameterList>,
    ) {
        let _ = (executable_fragment, formal_parameter_list);
    }

    /// Dart `_checkForExternalMethodWithBody(externalKeyword:, body:)`.
    pub(crate) fn check_for_external_method_with_body(
        &mut self,
        external_keyword: Option<TokenId>,
        body: Id<FunctionBody>,
    ) -> bool {
        if external_keyword.is_some() {
            if let Some(b) = self.ast.cast::<BlockFunctionBody>(body.raw()) {
                let left_bracket = self.ast[self.ast[b].block].left_bracket;
                self.report_at_token(diag::external_method_with_body(), left_bracket);
                return true;
            } else if let Some(b) = self.ast.cast::<ExpressionFunctionBody>(body.raw()) {
                let function_definition = self.ast[b].function_definition;
                self.report_at_token(diag::external_method_with_body(), function_definition);
                return true;
            }
        }
        false
    }

    /// Dart `_checkForTypeAnnotationDeferredClass(type)`.
    pub(crate) fn check_for_type_annotation_deferred_class(
        &mut self,
        type_: Option<Id<TypeAnnotation>>,
    ) {
        let Some(type_) = type_ else {
            return;
        };
        let Some(named_type) = self.ast.cast::<NamedType>(type_) else {
            return;
        };
        if self.named_type_is_deferred(named_type) {
            let type_name = self.ast.qualified_name(named_type);
            self.report_at(diag::type_annotation_deferred_class(&type_name), named_type);
        }
    }

    /// Dart `_checkForTypeParameterReferencedByStatic(name:, element:)`.
    pub(crate) fn check_for_type_parameter_referenced_by_static(
        &mut self,
        name: TokenId,
        element: Option<ElemRef>,
    ) {
        if self.enclosing_executable.in_static_method
            || self.this_context() == ThisContext::StaticFieldDeclaration
        {
            let Some(ElemRef::Base(element)) = element else {
                return;
            };
            if element.tag() == Tag::TypeParameter
                && self
                    .ctx
                    .element_data(element)
                    .and_then(|d| d.enclosing)
                    .is_some_and(|e| e.is::<InstanceElement>())
            {
                // The class's type parameters are not in scope for static
                // methods. However all other type parameters are legal (e.g.
                // the static method's type parameters, or a local function's
                // type parameters).
                self.report_at_token(diag::type_parameter_referenced_by_static(), name);
            }
        }
    }

    /// Dart `_checkForTypeParameterBoundRecursion(parameters)` (the
    /// top-level `checkForTypeParameterBoundRecursion`).
    pub(crate) fn check_for_type_parameter_bound_recursion(
        &mut self,
        parameters: &[Id<TypeParameter>],
    ) {
        let mut element_to_node: Option<indexmap::IndexMap<ElementId, Id<TypeParameter>>> = None;
        for &parameter in parameters {
            if self.ast[parameter].bound.is_none() {
                continue;
            }
            let element_to_node = element_to_node.get_or_insert_with(|| {
                let mut map = indexmap::IndexMap::new();
                for &parameter in parameters {
                    if let Some(element) = self.declared_element(parameter) {
                        map.insert(element, parameter);
                    }
                }
                map
            });

            let mut current = Some(parameter);
            let mut step = 0;
            while let Some(c) = current {
                let bound_node = self.ast[c].bound;
                current = None;
                if let Some(bound_node) = bound_node
                    && let Some(named_type) = self.ast.cast::<NamedType>(bound_node)
                {
                    let bound_type = self.annotation_type(named_type).unwrap_or(TypeId::INVALID);
                    let bound_type = self.type_system.extension_type_erasure(bound_type);
                    if let Some(element) = self.ctx.type_element(bound_type) {
                        current = element_to_node.get(&element).copied();
                    }
                }
                if step == parameters.len() {
                    if let Some(element) = self.declared_element(parameter) {
                        let display_name = self
                            .ctx
                            .element_name(element)
                            .unwrap_or("<unnamed>")
                            .to_string();
                        // This error can only occur if there is a bound, so
                        // we can safely assume `element.bound` is non-`null`.
                        let bound = element
                            .cast::<TypeParameterElement>()
                            .and_then(|e| self.ctx.get(e).bound.get())
                            .unwrap_or(TypeId::INVALID);
                        let name = self.ast[parameter].name;
                        self.report_at_token(
                            diag::type_parameter_supertype_of_its_bound(
                                &display_name,
                                type_arg(&self.ctx, bound),
                            ),
                            name,
                        );
                    }
                    break;
                }
                step += 1;
            }
        }
    }
}

/// Dart `List<String>.quotedAndCommaSeparatedWithAnd`.
fn quoted_and_comma_separated_with_and(items: &[String]) -> String {
    let quoted: Vec<String> = items.iter().map(|s| format!("'{s}'")).collect();
    match quoted.len() {
        0 => String::new(),
        1 => quoted[0].clone(),
        2 => format!("{} and {}", quoted[0], quoted[1]),
        n => format!("{}, and {}", quoted[..n - 1].join(", "), quoted[n - 1]),
    }
}

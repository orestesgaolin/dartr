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

use super::ErrorVerifier;

impl ErrorVerifier<'_> {
    /// Dart `visitFormalParameterList`.
    pub(super) fn visit_formal_parameter_list(&mut self, node: Id<FormalParameterList>) {
        self.visit_children(node);
    }

    /// Dart `visitFunctionDeclaration`.
    pub(super) fn visit_function_declaration(&mut self, node: Id<FunctionDeclaration>) {
        self.visit_children(node);
    }

    /// Dart `visitFunctionExpression`.
    pub(super) fn visit_function_expression(&mut self, node: Id<FunctionExpression>) {
        self.visit_children(node);
    }

    /// Dart `visitMethodDeclaration`.
    pub(super) fn visit_method_declaration(&mut self, node: Id<MethodDeclaration>) {
        self.visit_children(node);
    }

    /// Dart `visitNamedType`.
    pub(super) fn visit_named_type(&mut self, node: Id<NamedType>) {
        self.visit_children(node);
    }

    /// Dart `visitRegularFormalParameter`.
    pub(super) fn visit_regular_formal_parameter(&mut self, node: Id<RegularFormalParameter>) {
        self.visit_children(node);
    }

    /// Dart `visitTypeArgumentList`.
    pub(super) fn visit_type_argument_list(&mut self, node: Id<TypeArgumentList>) {
        self.visit_children(node);
    }

    /// Dart `visitTypeParameter`.
    pub(super) fn visit_type_parameter(&mut self, node: Id<TypeParameter>) {
        self.visit_children(node);
    }

    /// Dart `visitTypeParameterList`.
    pub(super) fn visit_type_parameter_list(&mut self, node: Id<TypeParameterList>) {
        self.visit_children(node);
    }

    // The helpers of this section (Dart private methods) to port here:
    // _checkForAmbiguousImport, _checkForAugmentationFormalParameters, _checkForConstVariableAugmentationByAccessor
    // _checkForDefaultValueAlreadySpecifiedInAugmentationChain, _checkForExtensionDeclaresAbstractMember, _checkForExtensionDeclaresMemberOfObject
    // _checkForExternalMethodWithBody, _checkForFunctionAlreadyComplete, _checkForFunctionBodyCompleteness
    // _checkForGenericFunctionType, _checkForMainFunction2, _checkForNonVoidReturnTypeForOperator
    // _checkForNonVoidReturnTypeForSetter, _checkForOptionalParameterInOperator, _checkForTypeAnnotationDeferredClass
    // _checkForTypeParameterBoundRecursion, _checkForTypeParameterReferencedByStatic, _checkForWrongNumberOfParametersForOperator
    // _checkForWrongTypeParameterVarianceInMethod, _checkUseOfCovariantInParameters, _checkUseOfDefaultValuesInParameters
    // _getLibraryName, _isWildcardSuperFormalPositionalParameter

    // Helpers that other sections call. STUB (D7): they report nothing yet;
    // keep the signatures (or update the callers) when porting them.

    /// Dart `_checkForAugmentationFormalParameters(executableFragment:,
    /// formalParameterList:)`.
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
        let _ = (external_keyword, body);
        false
    }

    /// Dart `_checkForTypeAnnotationDeferredClass(type)`.
    pub(crate) fn check_for_type_annotation_deferred_class(
        &mut self,
        type_: Option<Id<TypeAnnotation>>,
    ) {
        let _ = type_;
    }

    /// Dart `_checkForTypeParameterReferencedByStatic(name:, element:)`.
    pub(crate) fn check_for_type_parameter_referenced_by_static(
        &mut self,
        name: TokenId,
        element: Option<ElemRef>,
    ) {
        let _ = (name, element);
    }

    /// Dart `_checkForTypeParameterBoundRecursion(parameters)` (the
    /// top-level `checkForTypeParameterBoundRecursion`).
    pub(crate) fn check_for_type_parameter_bound_recursion(
        &mut self,
        parameters: &[Id<TypeParameter>],
    ) {
        let _ = parameters;
    }
}

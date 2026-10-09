// Dart source: pkg/analyzer/lib/src/generated/error_verifier.dart
// (ErrorVerifier section: type arguments, type parameters, formal parameters,
// functions and methods, variance, operators (D7))

//! An `ErrorVerifier` section (see the module documentation of
//! [`super`]). The `visit_x` methods are the Dart `visitX` overrides; the
//! body `self.visit_children(node)` is Dart `super.visitX(node)`.

use dartr_ast::*;
use dartr_element::{ElemRef, ElementId, FragmentId};
use dartr_syntax::TokenId;

use super::EnclosingExecutableContext;

impl EnclosingExecutableContext {
    /// Dart `EnclosingExecutableContext(element, isAsynchronous:,
    /// isGenerator:, catchErrorOnErrorReturnType:, thenOnErrorReturnType:)`.
    /// STUB (D7): `isConstConstructor`, `isGenerativeConstructor`,
    /// `inFactoryConstructor` and `inStaticMethod` are not computed yet.
    pub fn new(
        ctx: &dartr_element::Ctx<'_>,
        element: Option<ElementId>,
        is_asynchronous: bool,
        is_generator: bool,
    ) -> EnclosingExecutableContext {
        let _ = ctx;
        EnclosingExecutableContext {
            element,
            is_asynchronous,
            is_generator,
            ..EnclosingExecutableContext::empty()
        }
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

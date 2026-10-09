// Dart source: pkg/analyzer/lib/src/generated/error_verifier.dart
// (ErrorVerifier section: constructors, initializers, fields and variables (D6))

//! An `ErrorVerifier` section (see the module documentation of
//! [`super`]). The `visit_x` methods are the Dart `visitX` overrides; the
//! body `self.visit_children(node)` is Dart `super.visitX(node)`.

use dartr_ast::Entity;
use dartr_ast::*;
use dartr_element::{ElemRef, ElementId, FragmentId, Variance};
use dartr_syntax::TokenId;

use super::ErrorVerifier;

impl ErrorVerifier<'_> {
    /// Dart `visitConstructorDeclaration`.
    pub(super) fn visit_constructor_declaration(&mut self, node: Id<ConstructorDeclaration>) {
        self.visit_children(node);
    }

    /// Dart `visitConstructorFieldInitializer`.
    pub(super) fn visit_constructor_field_initializer(
        &mut self,
        node: Id<ConstructorFieldInitializer>,
    ) {
        self.visit_children(node);
    }

    /// Dart `visitDotShorthandConstructorInvocation`.
    pub(super) fn visit_dot_shorthand_constructor_invocation(
        &mut self,
        node: Id<DotShorthandConstructorInvocation>,
    ) {
        self.visit_children(node);
    }

    /// Dart `visitFieldDeclaration`.
    pub(super) fn visit_field_declaration(&mut self, node: Id<FieldDeclaration>) {
        self.visit_children(node);
    }

    /// Dart `visitFieldFormalParameter`.
    pub(super) fn visit_field_formal_parameter(&mut self, node: Id<FieldFormalParameter>) {
        self.visit_children(node);
    }

    /// Dart `visitInstanceCreationExpression`.
    pub(super) fn visit_instance_creation_expression(
        &mut self,
        node: Id<InstanceCreationExpression>,
    ) {
        self.visit_children(node);
    }

    /// Dart `visitPrimaryConstructorBody`.
    pub(super) fn visit_primary_constructor_body(&mut self, node: Id<PrimaryConstructorBody>) {
        self.visit_children(node);
    }

    /// Dart `visitPrimaryConstructorDeclaration`.
    pub(super) fn visit_primary_constructor_declaration(
        &mut self,
        node: Id<PrimaryConstructorDeclaration>,
    ) {
        self.visit_children(node);
    }

    /// Dart `visitRedirectingConstructorInvocation`.
    pub(super) fn visit_redirecting_constructor_invocation(
        &mut self,
        node: Id<RedirectingConstructorInvocation>,
    ) {
        self.visit_children(node);
    }

    /// Dart `visitSuperConstructorInvocation`.
    pub(super) fn visit_super_constructor_invocation(
        &mut self,
        node: Id<SuperConstructorInvocation>,
    ) {
        self.visit_children(node);
    }

    /// Dart `visitSuperFormalParameter`.
    pub(super) fn visit_super_formal_parameter(&mut self, node: Id<SuperFormalParameter>) {
        self.visit_children(node);
    }

    /// Dart `visitTopLevelVariableDeclaration`.
    pub(super) fn visit_top_level_variable_declaration(
        &mut self,
        node: Id<TopLevelVariableDeclaration>,
    ) {
        self.visit_children(node);
    }

    /// Dart `visitVariableDeclaration`.
    pub(super) fn visit_variable_declaration(&mut self, node: Id<VariableDeclaration>) {
        self.visit_children(node);
    }

    /// Dart `visitVariableDeclarationList`.
    pub(super) fn visit_variable_declaration_list(&mut self, node: Id<VariableDeclarationList>) {
        self.visit_children(node);
    }

    // The helpers of this section (Dart private methods) to port here:
    // _checkAugmentationWithoutDeclarationForInducedAccessors, _checkForAbstractOrExternalFieldConstructorInitializer, _checkForAbstractOrExternalVariableInitializer
    // _checkForAllRedirectConstructorErrorCodes, _checkForAugmentationInducedAccessorsAlreadyComplete, _checkForAugmentationReturnTypeMismatch
    // _checkForConflictingInitializerErrorCodes, _checkForConflictingPrimaryConstructorInitializers, _checkForConstConstructorWithNonConstSuper
    // _checkForConstConstructorWithNonFinalField, _checkForConstDeferredClass, _checkForConstOrNewWithAbstractClass
    // _checkForConstOrNewWithMixin, _checkForConstructorAugmentationModifierMismatch, _checkForConstVariableAugmentation
    // _checkForConstWithNonConst, _checkForConstWithUndefinedConstructor, _checkForDefaultValueAssignableAtType
    // _checkForExtensionDeclaresInstanceField, _checkForExtensionTypeConstructorWithSuperInvocation, _checkForExtensionTypeDeclaresInstanceField
    // _checkForFactoryBodyCompleteness, _checkForFieldInitializingFormalRedirectingConstructor, _checkForIncompleteInducedAccessors
    // _checkForInvalidField, _checkForInvalidGenerativeConstructorReference, _checkForInvalidModifierOnBody
    // _checkForLateFinalFieldWithConstConstructor, _checkForNewWithUndefinedConstructor, _checkForNonConstGenerativeEnumConstructor
    // _checkForNonFinalFieldInEnum, _checkForNonRedirectingGenerativeConstructorWithPrimary, _checkForRecursiveConstructorRedirect
    // _checkForRecursiveFactoryRedirect, _checkForRedirectingConstructorErrorCodes, _checkForRedirectToNonConstConstructor
    // _checkForReturnInGenerativeConstructor, _checkForUndefinedConstructorInInitializerImplicit, _checkForUndefinedConstructorInInitializerImplicitConstructor
    // _checkForValidField, _checkForWrongTypeParameterVarianceInField, _checkForWrongVariancePosition
    // _checkPrivateOptionalParameter, _hasRedirectingFactoryConstructorCycle, _validateConstructorBodyAllowed
    // _withEnclosingExecutable, _withThisContext

    // Helpers that other sections call. STUB (D6): they report nothing yet;
    // keep the signatures (or update the callers) when porting them.

    /// Dart `_checkForAugmentationReturnTypeMismatch(fragment:,
    /// returnTypeNode:, errorEntity:)`.
    pub(crate) fn check_for_augmentation_return_type_mismatch(
        &mut self,
        fragment: FragmentId,
        return_type_node: Option<Id<TypeAnnotation>>,
        error_entity: Entity,
    ) {
        let _ = (fragment, return_type_node, error_entity);
    }

    /// Dart `_checkForConstVariableAugmentation(errorToken:, fragment:)`.
    pub(crate) fn check_for_const_variable_augmentation(
        &mut self,
        error_token: TokenId,
        fragment: FragmentId,
    ) -> bool {
        let _ = (error_token, fragment);
        false
    }

    /// Dart `_checkForDefaultValueAssignableAtType(node)`.
    pub(crate) fn check_for_default_value_assignable_at_type(&mut self, node: Id<FormalParameter>) {
        let _ = node;
    }

    /// Dart `_checkForInvalidGenerativeConstructorReference(node,
    /// constructorElement)`.
    pub(crate) fn check_for_invalid_generative_constructor_reference(
        &mut self,
        node: NodeId,
        constructor_element: Option<ElemRef>,
    ) {
        let _ = (node, constructor_element);
    }

    /// Dart `_checkForWrongVariancePosition(variance, typeParameter,
    /// errorTarget)`.
    pub(crate) fn check_for_wrong_variance_position(
        &mut self,
        variance: Variance,
        type_parameter: ElementId,
        error_target: Entity,
    ) {
        let _ = (variance, type_parameter, error_target);
    }

    /// Dart `_checkPrivateOptionalParameter(node)`.
    pub(crate) fn check_private_optional_parameter(&mut self, node: Id<FormalParameter>) {
        let _ = node;
    }
}

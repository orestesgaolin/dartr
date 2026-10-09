// Dart source: pkg/analyzer/lib/src/generated/error_verifier.dart
// (ErrorVerifier section: class, enum, mixin, extension, extension type and type alias
// declarations, directives, the compilation unit (D4))

//! An `ErrorVerifier` section (see the module documentation of
//! [`super`]). The `visit_x` methods are the Dart `visitX` overrides; the
//! body `self.visit_children(node)` is Dart `super.visitX(node)`.

use dartr_ast::*;
use dartr_diagnostics::LocatableDiagnostic;
use dartr_element::FragmentId;
use dartr_syntax::TokenId;

use super::ErrorVerifier;

impl ErrorVerifier<'_> {
    /// Dart `visitClassDeclaration`.
    pub(super) fn visit_class_declaration(&mut self, node: Id<ClassDeclaration>) {
        self.visit_children(node);
    }

    /// Dart `visitClassTypeAlias`.
    pub(super) fn visit_class_type_alias(&mut self, node: Id<ClassTypeAlias>) {
        self.visit_children(node);
    }

    /// Dart `visitComment`.
    pub(super) fn visit_comment(&mut self, node: Id<Comment>) {
        self.visit_children(node);
    }

    /// Dart `visitCompilationUnit`.
    pub(super) fn visit_compilation_unit(&mut self, node: Id<CompilationUnit>) {
        self.visit_children(node);
    }

    /// Dart `visitEnumConstantDeclaration`.
    pub(super) fn visit_enum_constant_declaration(&mut self, node: Id<EnumConstantDeclaration>) {
        self.visit_children(node);
    }

    /// Dart `visitEnumDeclaration`.
    pub(super) fn visit_enum_declaration(&mut self, node: Id<EnumDeclaration>) {
        self.visit_children(node);
    }

    /// Dart `visitExportDirective`.
    pub(super) fn visit_export_directive(&mut self, node: Id<ExportDirective>) {
        self.visit_children(node);
    }

    /// Dart `visitExtensionDeclaration`.
    pub(super) fn visit_extension_declaration(&mut self, node: Id<ExtensionDeclaration>) {
        self.visit_children(node);
    }

    /// Dart `visitExtensionTypeDeclaration`.
    pub(super) fn visit_extension_type_declaration(&mut self, node: Id<ExtensionTypeDeclaration>) {
        self.visit_children(node);
    }

    /// Dart `visitFunctionTypeAlias`.
    pub(super) fn visit_function_type_alias(&mut self, node: Id<FunctionTypeAlias>) {
        self.visit_children(node);
    }

    /// Dart `visitGenericTypeAlias`.
    pub(super) fn visit_generic_type_alias(&mut self, node: Id<GenericTypeAlias>) {
        self.visit_children(node);
    }

    /// Dart `visitImportDirective`.
    pub(super) fn visit_import_directive(&mut self, node: Id<ImportDirective>) {
        self.visit_children(node);
    }

    /// Dart `visitMixinDeclaration`.
    pub(super) fn visit_mixin_declaration(&mut self, node: Id<MixinDeclaration>) {
        self.visit_children(node);
    }

    /// Dart `visitNativeClause`.
    pub(super) fn visit_native_clause(&mut self, node: Id<NativeClause>) {
        self.visit_children(node);
    }

    // The helpers of this section (Dart private methods) to port here:
    // _checkAugmentationWithoutDeclaration, _checkClassInheritance, _checkDeferredPrefixCollision
    // _checkEnumConstantSameAsEnclosing, _checkForAllMixinErrorCodes, _checkForAmbiguousExport
    // _checkForAugmentationExtendsClauseAlreadyPresent, _checkForAugmentationModifierMismatch, _checkForAugmentationTypeParameters
    // _checkForBadFunctionUse, _checkForBaseClassOrMixinImplementedOutsideOfLibrary, _checkForBuiltInIdentifierAsName
    // _checkForClassAugmentationModifierMismatch, _checkForClassUsedAsMixinDeclaresGenerativeConstructor, _checkForConflictingClassMembers
    // _checkForConflictingClassTypeVariableErrorCodes, _checkForConflictingEnumTypeVariableErrorCodes, _checkForConflictingExtensionTypeTypeVariableErrorCodes
    // _checkForConflictingExtensionTypeVariableErrorCodes, _checkForConflictingGenerics, _checkForDeferredImportOfExtensions
    // _checkForDeferredPrefixCollisions, _checkForEnumInstantiatedToBoundsIsNotWellBounded, _checkForEnumWithNameValues
    // _checkForExportInternalLibrary, _checkForExtendsDeferredClass, _checkForExtendsDisallowedClass
    // _checkForExtendsOrImplementsDeferredClass, _checkForExtendsOrImplementsDisallowedClass, _checkForExtensionTypeImplementsDeferred
    // _checkForExtensionTypeImplementsItself, _checkForExtensionTypeMemberConflicts, _checkForExtensionTypeRepresentationDependsOnItself
    // _checkForExtensionTypeRepresentationErrorCodes, _checkForExtensionTypeRepresentationTypeBottom, _checkForExtensionTypeWithAbstractMember
    // _checkForFinalSupertypeOutsideOfLibrary, _checkForIllegalLanguageOverride, _checkForImplementsClauseErrorCodes
    // _checkForImportInternalLibrary, _checkForInterfaceClassOrMixinSuperclassOutsideOfLibrary, _checkForMainFunction1
    // _checkForMixinAugmentationModifierMismatch, _checkForMixinClassErrorCodes, _checkForMixinInheritsNotFromObject
    // _checkForMixinSuperclassConstraints, _checkForMixinSuperInvokedMembers, _checkForMixinWithConflictingPrivateMember
    // _checkForMultiplePrimaryConstructorBodyDeclarations, _checkForNoDefaultSuperConstructorImplicit, _checkForNoGenerativeConstructorsInSuperclass
    // _checkForNonCovariantTypeParameterPositionInRepresentationType, _checkForNotInitializedFieldDeclaration, _checkForNotInitializedFieldDeclarations
    // _checkForOnClauseErrorCodes, _checkForRepeatedType, _checkForSealedSupertypeOutsideOfLibrary
    // _checkForTypeAliasCannotReferenceItself, _checkForWrongTypeParameterVarianceInSuperinterfaces, _checkImplementsSuperClass
    // _checkImplementsSuperClassConstraint, _checkMixinInheritance, _checkMixinsSuperClass
    // _isWasm, _mayIgnoreClassModifiers, _reportForMultipleCombinators

    // Helpers that other sections call. STUB (D4): they report nothing yet;
    // keep the signatures (or update the callers) when porting them.

    /// Dart `_checkAugmentationWithoutDeclaration(errorToken, fragment)`.
    pub(crate) fn check_augmentation_without_declaration(
        &mut self,
        error_token: Option<TokenId>,
        fragment: FragmentId,
    ) {
        let _ = (error_token, fragment);
    }

    /// Dart `_checkForAugmentationModifierMismatch(augmentKeyword:,
    /// inAugmentation:, inIntroductory:, modifierToken:, modifierName:)`.
    pub(crate) fn check_for_augmentation_modifier_mismatch(
        &mut self,
        augment_keyword: TokenId,
        in_augmentation: bool,
        in_introductory: bool,
        modifier_token: Option<TokenId>,
        modifier_name: &str,
    ) {
        let _ = (
            augment_keyword,
            in_augmentation,
            in_introductory,
            modifier_token,
            modifier_name,
        );
    }

    /// Dart `_checkForAugmentationTypeParameters(fragment:,
    /// firstTypeParameters:, nameOrKeywordToken:, typeParameterList:)`.
    pub(crate) fn check_for_augmentation_type_parameters(
        &mut self,
        fragment: FragmentId,
        first_type_parameters: &[FragmentId],
        name_or_keyword_token: TokenId,
        type_parameter_list: Option<Id<TypeParameterList>>,
    ) {
        let _ = (
            fragment,
            first_type_parameters,
            name_or_keyword_token,
            type_parameter_list,
        );
    }

    /// Dart `_checkForBuiltInIdentifierAsName(token, code)`; [code] builds
    /// the diagnostic from the name.
    pub(crate) fn check_for_built_in_identifier_as_name(
        &mut self,
        token: TokenId,
        code: fn(&str) -> LocatableDiagnostic,
    ) {
        let _ = (token, code);
    }

    /// Dart `_checkForMainFunction1(nameToken, declaredFragment)`.
    pub(crate) fn check_for_main_function1(
        &mut self,
        name_token: TokenId,
        declared_fragment: FragmentId,
    ) {
        let _ = (name_token, declared_fragment);
    }
}

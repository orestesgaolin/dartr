// Dart source: pkg/analyzer/lib/src/generated/error_verifier.dart
// (ErrorVerifier section: class, enum, mixin, extension, extension type and type alias
// declarations, directives, the compilation unit (D4))

//! An `ErrorVerifier` section (see the module documentation of
//! [`super`]). The `visit_x` methods are the Dart `visitX` overrides; the
//! body `self.visit_children(node)` is Dart `super.visitX(node)`.
//!
//! Not ported (no data in the Rust model yet):
//! - `_checkForIllegalLanguageOverride`: `AnalysisOptions` has no
//!   `sourceLanguageConstraint`.
//! - `_checkForEnumInstantiatedToBoundsIsNotWellBounded`:
//!   `TypeSystem.isWellBounded` is not ported.
//! - The context messages of the diagnostics (the Rust diagnostics have no
//!   context messages yet).

use dartr_ast::*;
use dartr_diagnostics::{LocatableDiagnostic, diag};
use dartr_element::diagnostics::{element_arg, library_fragment_of, type_arg, type_display_string};
use dartr_element::{
    AnyElement, DirectiveUri, EId, ElemRef, ElementFlags, ElementId, ExtensionTypeElement, FId,
    FragmentFlags, FragmentId, InterfaceElement, LibraryElement, MixinFragment,
    NamespaceCombinator, Tag, TypeId, TypeKind, TypeParameterElement, TypeProvider,
};
use dartr_flow::shared_type::Variance;
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_syntax::{Keyword, TokenId, TokenType};
use dartr_typesystem::TypeExt;
use dartr_typesystem::inheritance_manager3::{Conflict, GetMemberOptions, Name};
use dartr_typesystem::{lookup, member};
use indexmap::{IndexMap, IndexSet};

use super::ErrorVerifier;
use crate::error::{
    correct_override, duplicate_definition_verifier, getter_setter_types_verifier,
    required_parameters_verifier, type_arguments_verifier,
};

/// The set of [ErrorVerifier::check_for_repeated_type] (Dart
/// `libraryContext.setOfImplements` / `setOfOn`).
#[derive(Clone, Copy)]
enum RepeatedTypeSet {
    Implements,
    On,
}

impl ErrorVerifier<'_> {
    /// Dart `visitClassDeclaration`.
    pub(super) fn visit_class_declaration(&mut self, node: Id<ClassDeclaration>) {
        let ctx = self.ctx;
        let ast = self.ast;
        let n = &ast[node];
        let Some(declared_fragment) = self.declared_fragment(node) else {
            self.visit_children(node);
            return;
        };
        let Some(augmented) = self.element_of(declared_fragment) else {
            self.visit_children(node);
            return;
        };
        let declaration_fragment = self.first_fragment(augmented);
        self.enclosing_class = augmented.cast::<InterfaceElement>();

        let type_name = name_part_type_name(ast, n.name_part);
        let type_parameters = name_part_type_parameters(ast, n.name_part);
        self.check_augmentation_without_declaration(n.augment_keyword, declared_fragment);
        let first_type_parameters = self.fragment_type_parameters(declaration_fragment);
        self.check_for_augmentation_type_parameters(
            declared_fragment,
            &first_type_parameters,
            type_name,
            type_parameters,
        );
        self.check_for_augmentation_extends_clause_already_present(node, declared_fragment);
        self.check_for_class_augmentation_modifier_mismatch(node, declared_fragment);

        let members = class_body_members(ast, n.body.raw());
        if !ctx.is_element(augmented, "dart.core", "Function") {
            self.check_for_built_in_identifier_as_name(
                type_name,
                diag::built_in_identifier_as_type_name,
            );
        }
        self.check_for_conflicting_class_type_variable_error_codes();
        let superclass = n.extends_clause.map(|c| ast[c].superclass);
        let implements_clause = n.implements_clause;
        let with_clause = n.with_clause;

        // Only do error checks on the clause nodes if there is a non-null clause
        if implements_clause.is_some() || superclass.is_some() || with_clause.is_some() {
            let more_checks = self.check_class_inheritance(
                augmented,
                node.raw(),
                type_name,
                superclass,
                with_clause,
                implements_clause,
            );
            if more_checks {
                self.check_for_no_default_super_constructor_implicit(augmented);
            }
        }

        if n.native_clause.is_none() {
            self.library_context
                .constructor_fields_verifier
                .add_constructors(0, augmented, &members, n.name_part.raw());
        }

        self.check_for_conflicting_class_members(declared_fragment);
        self.check_for_not_initialized_field_declarations(augmented, &members);
        self.check_for_bad_function_use(superclass, implements_clause, with_clause);
        self.check_for_wrong_type_parameter_variance_in_superinterfaces();
        self.check_for_main_function1(type_name, declared_fragment);
        self.check_for_mixin_class_error_codes(node.raw(), &members, superclass, with_clause);
        self.check_for_multiple_primary_constructor_body_declarations(&members);

        let getters = self.instance_getters(augmented);
        getter_setter_types_verifier::check_static_getters(self, &getters);

        self.visit_children(node);
        self.enclosing_class = None;
    }

    /// Dart `visitClassTypeAlias`.
    pub(super) fn visit_class_type_alias(&mut self, node: Id<ClassTypeAlias>) {
        let ast = self.ast;
        let n = &ast[node];
        self.check_for_built_in_identifier_as_name(
            n.name,
            diag::built_in_identifier_as_typedef_name,
        );
        if let Some(fragment) = self.declared_fragment(node)
            && let Some(element) = self.element_of(fragment)
        {
            self.enclosing_class = element.cast::<InterfaceElement>();
            self.check_class_inheritance(
                element,
                node.raw(),
                n.name,
                Some(n.superclass),
                Some(n.with_clause),
                n.implements_clause,
            );
            self.check_for_main_function1(n.name, fragment);
            self.check_for_mixin_class_error_codes(
                node.raw(),
                &[],
                Some(n.superclass),
                Some(n.with_clause),
            );
            self.check_for_bad_function_use(
                Some(n.superclass),
                n.implements_clause,
                Some(n.with_clause),
            );
            self.check_for_wrong_type_parameter_variance_in_superinterfaces();
            self.enclosing_class = None;
        }
        self.visit_children(node);
    }

    /// Dart `visitComment`.
    pub(super) fn visit_comment(&mut self, node: Id<Comment>) {
        self.is_in_comment = true;
        self.visit_children(node);
        self.is_in_comment = false;
    }

    /// Dart `visitCompilationUnit`.
    pub(super) fn visit_compilation_unit(&mut self, node: Id<CompilationUnit>) {
        duplicate_definition_verifier::check_unit(self, node);
        self.check_for_deferred_prefix_collisions(node);
        // Dart `_checkForIllegalLanguageOverride(node)`: not ported (see the
        // module documentation).
        let library = self.current_library();
        let getters: Vec<ElementId> = self
            .ctx
            .get(library)
            .getters
            .iter()
            .map(|g| g.raw())
            .collect();
        getter_setter_types_verifier::check_static_getters(self, &getters);
        self.visit_children(node);
    }

    /// Dart `visitEnumConstantDeclaration`.
    pub(super) fn visit_enum_constant_declaration(&mut self, node: Id<EnumConstantDeclaration>) {
        self.check_enum_constant_same_as_enclosing(node);
        if let Some(fragment) = self.declared_fragment(node) {
            self.check_for_const_variable_augmentation(self.ast[node].name, fragment);
        }
        required_parameters_verifier::visit_enum_constant_declaration(self, node);
        type_arguments_verifier::check_enum_constant_declaration(self, node);
        self.visit_children(node);
    }

    /// Dart `visitEnumDeclaration`.
    pub(super) fn visit_enum_declaration(&mut self, node: Id<EnumDeclaration>) {
        let ast = self.ast;
        let n = &ast[node];
        let Some(declared_fragment) = self.declared_fragment(node) else {
            self.visit_children(node);
            return;
        };
        let Some(element) = self.element_of(declared_fragment) else {
            self.visit_children(node);
            return;
        };
        let type_name = name_part_type_name(ast, n.name_part);
        self.check_augmentation_without_declaration(n.augment_keyword, declared_fragment);
        let first_fragment = self.first_fragment(element);
        let first_type_parameters = self.fragment_type_parameters(first_fragment);
        self.check_for_augmentation_type_parameters(
            declared_fragment,
            &first_type_parameters,
            type_name,
            name_part_type_parameters(ast, n.name_part),
        );

        self.enclosing_class = element.cast::<InterfaceElement>();

        self.check_for_enum_with_name_values(type_name);
        self.check_for_built_in_identifier_as_name(
            type_name,
            diag::built_in_identifier_as_type_name,
        );
        self.check_for_conflicting_enum_type_variable_error_codes(declared_fragment);
        let implements_clause = n.implements_clause;
        let with_clause = n.with_clause;
        if implements_clause.is_some() || with_clause.is_some() {
            self.check_class_inheritance(
                element,
                node.raw(),
                type_name,
                None,
                with_clause,
                implements_clause,
            );
        }

        if !self.fragment_has(declared_fragment, FragmentFlags::FRAGMENT_IS_AUGMENTATION) {
            let has_constants = self
                .ctx
                .instance(EId::from_raw(element))
                .fields
                .iter()
                .any(|f| {
                    self.first_fragment_has(f.raw(), FragmentFlags::FIELD_FRAGMENT_IS_ENUM_CONSTANT)
                });
            if !has_constants {
                self.report_at_token(diag::enum_without_constants(), type_name);
            }
        }

        let members = enum_body_members(ast, n.body.raw());
        self.library_context
            .constructor_fields_verifier
            .add_constructors(0, element, &members, n.name_part.raw());
        self.check_for_not_initialized_field_declarations(element, &members);
        self.check_for_wrong_type_parameter_variance_in_superinterfaces();
        self.check_for_main_function1(type_name, declared_fragment);
        // Dart `_checkForEnumInstantiatedToBoundsIsNotWellBounded`: not
        // ported (see the module documentation).
        self.check_for_multiple_primary_constructor_body_declarations(&members);

        let getters = self.instance_getters(element);
        getter_setter_types_verifier::check_static_getters(self, &getters);

        self.visit_children(node);
        self.enclosing_class = None;
    }

    /// Dart `visitExportDirective`.
    pub(super) fn visit_export_directive(&mut self, node: Id<ExportDirective>) {
        let index = self.directive_index::<ExportDirective>(node.raw());
        let ctx = self.ctx;
        let unit = ctx.fragment(self.current_unit());
        if let Some(export) = index.and_then(|i| unit.library_exports.get(i)) {
            let exported_library = match &export.directive.uri {
                DirectiveUri::Library { library, .. } => Some(*library),
                _ => None,
            };
            self.check_for_ambiguous_export(node, &export.combinators, exported_library);
            self.check_for_export_internal_library(node, exported_library);
        }
        self.report_for_multiple_combinators(self.ast[node].combinators);
        self.visit_children(node);
    }

    /// Dart `visitExtensionDeclaration`.
    pub(super) fn visit_extension_declaration(&mut self, node: Id<ExtensionDeclaration>) {
        let ast = self.ast;
        let n = &ast[node];
        let Some(declared_fragment) = self.declared_fragment(node) else {
            self.visit_children(node);
            return;
        };
        self.check_augmentation_without_declaration(n.augment_keyword, declared_fragment);
        let Some(element) = self.element_of(declared_fragment) else {
            self.visit_children(node);
            return;
        };
        let first_fragment = self.first_fragment(element);
        let first_type_parameters = self.fragment_type_parameters(first_fragment);
        self.check_for_augmentation_type_parameters(
            declared_fragment,
            &first_type_parameters,
            n.name.unwrap_or(n.extension_keyword),
            n.type_parameters,
        );

        self.enclosing_extension = element.cast();
        self.check_for_conflicting_extension_type_variable_error_codes();
        let members = class_body_members(ast, n.body.raw());
        self.check_for_not_initialized_field_declarations(element, &members);
        getter_setter_types_verifier::check_extension(self, element);
        if let Some(name) = n.name {
            self.check_for_built_in_identifier_as_name(
                name,
                diag::built_in_identifier_as_extension_name,
            );
        }
        self.visit_children(node);
        self.enclosing_extension = None;
    }

    /// Dart `visitExtensionTypeDeclaration`.
    pub(super) fn visit_extension_type_declaration(&mut self, node: Id<ExtensionTypeDeclaration>) {
        let ast = self.ast;
        let n = &ast[node];
        let Some(declared_fragment) = self.declared_fragment(node) else {
            self.visit_children(node);
            return;
        };
        self.check_augmentation_without_declaration(n.augment_keyword, declared_fragment);
        let Some(element) = self.element_of(declared_fragment) else {
            self.visit_children(node);
            return;
        };
        let type_name = name_part_type_name(ast, n.name_part);
        let first_fragment = self.first_fragment(element);
        let first_type_parameters = self.fragment_type_parameters(first_fragment);
        self.check_for_augmentation_type_parameters(
            declared_fragment,
            &first_type_parameters,
            type_name,
            name_part_type_parameters(ast, n.name_part),
        );

        self.enclosing_class = element.cast::<InterfaceElement>();

        self.check_for_built_in_identifier_as_name(
            type_name,
            diag::built_in_identifier_as_extension_type_name,
        );
        self.check_for_conflicting_extension_type_type_variable_error_codes(declared_fragment);
        let members = class_body_members(ast, n.body.raw());
        self.check_for_not_initialized_field_declarations(element, &members);

        let interfaces = n.implements_clause.map(|c| ast[c].interfaces);
        self.check_for_repeated_type(
            element,
            RepeatedTypeSet::Implements,
            interfaces,
            diag::implements_repeated,
        );
        self.check_for_conflicting_class_members(declared_fragment);
        self.check_for_conflicting_generics(declared_fragment, type_name);
        self.library_context
            .constructor_fields_verifier
            .add_constructors(0, element, &members, n.name_part.raw());

        if let Some(ext) = element.cast::<ExtensionTypeElement>() {
            self.check_for_non_covariant_type_parameter_position_in_representation_type(node, ext);
            self.check_for_extension_type_representation_depends_on_itself(node, ext);
            self.check_for_extension_type_representation_type_bottom(node, ext);
            self.check_for_extension_type_implements_deferred(node);
            self.check_for_extension_type_implements_itself(node, ext);
            self.check_for_extension_type_member_conflicts(node, ext);
        }
        self.check_for_extension_type_with_abstract_member(node, &members);
        self.check_for_extension_type_representation_error_codes(node);
        self.check_for_wrong_type_parameter_variance_in_superinterfaces();
        self.check_for_multiple_primary_constructor_body_declarations(&members);

        getter_setter_types_verifier::check_extension_type(self, element);

        self.visit_children(node);
        self.enclosing_class = None;
    }

    /// Dart `visitFunctionTypeAlias`.
    pub(super) fn visit_function_type_alias(&mut self, node: Id<FunctionTypeAlias>) {
        let name = self.ast[node].name;
        self.check_for_built_in_identifier_as_name(name, diag::built_in_identifier_as_typedef_name);
        if let Some(fragment) = self.declared_fragment(node) {
            self.check_for_main_function1(name, fragment);
            self.check_for_type_alias_cannot_reference_itself(name, fragment);
        }
        self.visit_children(node);
    }

    /// Dart `visitGenericTypeAlias`.
    pub(super) fn visit_generic_type_alias(&mut self, node: Id<GenericTypeAlias>) {
        let name = self.ast[node].name;
        self.check_for_built_in_identifier_as_name(name, diag::built_in_identifier_as_typedef_name);
        if let Some(fragment) = self.declared_fragment(node) {
            self.check_for_main_function1(name, fragment);
            self.check_for_type_alias_cannot_reference_itself(name, fragment);
        }
        self.visit_children(node);
    }

    /// Dart `visitImportDirective`.
    pub(super) fn visit_import_directive(&mut self, node: Id<ImportDirective>) {
        let ast = self.ast;
        if let Some(prefix) = ast[node].prefix {
            self.check_for_built_in_identifier_as_name(
                ast[prefix].token,
                diag::built_in_identifier_as_prefix_name,
            );
        }
        let index = self.directive_index::<ImportDirective>(node.raw());
        let ctx = self.ctx;
        let unit = ctx.fragment(self.current_unit());
        if let Some(import) = index.and_then(|i| unit.library_imports.get(i)) {
            let imported_library = match &import.directive.uri {
                DirectiveUri::Library { library, .. } => Some(*library),
                _ => None,
            };
            self.check_for_import_internal_library(node, imported_library);
            let is_deferred = import.prefix.is_some_and(|p| ctx.fragment(p).is_deferred);
            if is_deferred {
                self.check_for_deferred_import_of_extensions(
                    node,
                    &import.combinators,
                    imported_library,
                );
            }
        }
        self.report_for_multiple_combinators(ast[node].combinators);
        self.visit_children(node);
    }

    /// Dart `visitMixinDeclaration`.
    pub(super) fn visit_mixin_declaration(&mut self, node: Id<MixinDeclaration>) {
        let ast = self.ast;
        let n = &ast[node];
        let Some(declared_fragment) = self.declared_fragment(node) else {
            self.visit_children(node);
            return;
        };
        self.check_augmentation_without_declaration(n.augment_keyword, declared_fragment);
        let Some(element) = self.element_of(declared_fragment) else {
            self.visit_children(node);
            return;
        };
        let first_fragment = self.first_fragment(element);
        let first_type_parameters = self.fragment_type_parameters(first_fragment);
        self.check_for_augmentation_type_parameters(
            declared_fragment,
            &first_type_parameters,
            n.name,
            n.type_parameters,
        );
        self.check_for_mixin_augmentation_modifier_mismatch(node, declared_fragment);

        self.enclosing_class = element.cast::<InterfaceElement>();

        self.check_for_built_in_identifier_as_name(n.name, diag::built_in_identifier_as_type_name);
        self.check_for_conflicting_class_type_variable_error_codes();

        // Only do error checks only if there is a non-null clause.
        if n.on_clause.is_some() || n.implements_clause.is_some() {
            self.check_mixin_inheritance(element, declared_fragment, node);
        }

        self.check_for_conflicting_class_members(declared_fragment);
        let members = class_body_members(ast, n.body.raw());
        self.check_for_not_initialized_field_declarations(element, &members);
        self.check_for_main_function1(n.name, first_fragment);
        self.check_for_wrong_type_parameter_variance_in_superinterfaces();
        self.visit_children(node);
        self.enclosing_class = None;
    }

    /// Dart `visitNativeClause`.
    pub(super) fn visit_native_clause(&mut self, node: Id<NativeClause>) {
        if !self.is_in_system_library {
            self.report_at(diag::native_clause_in_non_sdk_code(), node);
        }
        self.visit_children(node);
    }

    // ------------------------------------------------------------ checks

    /// Dart `_checkClassInheritance`.
    fn check_class_inheritance(
        &mut self,
        declaration: ElementId,
        node: NodeId,
        name_token: TokenId,
        superclass: Option<Id<NamedType>>,
        with_clause: Option<Id<WithClause>>,
        implements_clause: Option<Id<ImplementsClause>>,
    ) -> bool {
        let ast = self.ast;
        // Only check for all of the inheritance logic around clauses if there
        // isn't an error code such as "Cannot extend double" already on the
        // class.
        if !self.check_for_extends_disallowed_class(superclass)
            && !self.check_for_implements_clause_error_codes(implements_clause)
            && !self.check_for_all_mixin_error_codes(with_clause)
            && !self.check_for_no_generative_constructors_in_superclass(superclass)
        {
            self.check_for_extends_deferred_class(superclass);
            self.check_for_repeated_type(
                declaration,
                RepeatedTypeSet::Implements,
                implements_clause.map(|c| ast[c].interfaces),
                diag::implements_repeated,
            );
            self.check_implements_super_class(implements_clause);
            self.check_mixins_super_class(with_clause);
            self.check_for_mixin_with_conflicting_private_member(with_clause, superclass);
            if let Some(fragment) = self.declared_fragment(node) {
                self.check_for_conflicting_generics(fragment, name_token);
            }
            self.check_for_base_class_or_mixin_implemented_outside_of_library(implements_clause);
            self.check_for_interface_class_or_mixin_superclass_outside_of_library(superclass);
            self.check_for_final_supertype_outside_of_library(superclass, implements_clause, None);
            let mut supertypes: Vec<Id<NamedType>> = Vec::new();
            supertypes.extend(superclass);
            if let Some(w) = with_clause {
                supertypes.extend(ast.list(ast[w].mixin_types).iter().copied());
            }
            if let Some(i) = implements_clause {
                supertypes.extend(ast.list(ast[i].interfaces).iter().copied());
            }
            self.check_for_sealed_supertype_outside_of_library(&supertypes);
            return true;
        }
        false
    }

    /// Dart `_checkDeferredPrefixCollision`.
    fn check_deferred_prefix_collision(&mut self, directives: &[Id<ImportDirective>]) {
        if directives.len() > 1 {
            for &directive in directives {
                if let Some(deferred) = self.ast[directive].deferred_keyword {
                    self.report_at_token(diag::shared_deferred_prefix(), deferred);
                }
            }
        }
    }

    /// Dart `_checkEnumConstantSameAsEnclosing`.
    fn check_enum_constant_same_as_enclosing(&mut self, node: Id<EnumConstantDeclaration>) {
        let name = self.ast[node].name;
        let enclosing_name = self
            .enclosing_class
            .and_then(|c| self.ctx.element_name(c.raw()));
        if Some(self.ast.tokens.lexeme(name)) == enclosing_name {
            self.report_at_token(diag::enum_constant_same_name_as_enclosing(), name);
        }
    }

    /// Dart `_checkForAllMixinErrorCodes`.
    fn check_for_all_mixin_error_codes(&mut self, with_clause: Option<Id<WithClause>>) -> bool {
        let Some(with_clause) = with_clause else {
            return false;
        };
        let ctx = self.ctx;
        let ast = self.ast;
        let mut problem_reported = false;
        let mut mixin_type_index: i32 = -1;
        let mixin_types = ast.list(ast[with_clause].mixin_types);
        for (mixin_name_index, &mixin_name) in mixin_types.iter().enumerate() {
            let mixin_type = self.named_type_type(mixin_name);
            let Some(mixin_element) = ctx.interface_element(mixin_type) else {
                continue;
            };
            mixin_type_index += 1;
            if self.check_for_extends_or_implements_disallowed_class(mixin_name)
                && !ctx.is_dart_core_enum(mixin_type)
            {
                problem_reported = true;
                continue;
            }
            if self.check_for_extends_or_implements_deferred_class(
                mixin_name,
                diag::mixin_deferred_class(),
            ) {
                problem_reported = true;
            }
            let e = mixin_element.raw();
            let tag = e.tag();
            if tag == Tag::ExtensionType {
                // Already reported.
            } else if tag == Tag::Mixin {
                if self.check_for_mixin_superclass_constraints(mixin_name_index, mixin_name)
                    || self.check_for_mixin_super_invoked_members(
                        mixin_type_index,
                        mixin_name,
                        mixin_element,
                        mixin_type,
                    )
                {
                    problem_reported = true;
                }
            } else if tag == Tag::Class
                && !self.is_mixin_class(e)
                && self.library_of(e).is_some_and(|l| {
                    crate::scope::library_feature_enabled(&ctx, l, ExperimentalFlag::ClassModifiers)
                })
                && !self.may_ignore_class_modifiers(self.library_of(e))
            {
                let name = self.name_of(e);
                self.report_at(diag::class_used_as_mixin(&name), mixin_name);
            } else {
                if tag == Tag::Class
                    && !self.is_mixin_class(e)
                    && self.check_for_class_used_as_mixin_declares_generative_constructor(
                        mixin_name,
                        mixin_element,
                    )
                {
                    problem_reported = true;
                }
                if self.check_for_mixin_inherits_not_from_object(mixin_name, mixin_element) {
                    problem_reported = true;
                }
            }
        }
        problem_reported
    }

    /// Dart `_checkForAmbiguousExport`.
    fn check_for_ambiguous_export(
        &mut self,
        directive: Id<ExportDirective>,
        combinators: &[NamespaceCombinator],
        exported_library: Option<EId<LibraryElement>>,
    ) {
        let Some(exported_library) = exported_library else {
            return;
        };
        let ctx = self.ctx;
        for (name, element) in self.export_namespace_for_directive(exported_library, combinators) {
            let prev_element = self.library_context.exported_elements.get(&name).copied();
            match prev_element {
                Some(prev) if prev != element => {
                    let first_uri = self
                        .library_of(prev)
                        .map(|l| ctx.library_uri(l))
                        .unwrap_or_default();
                    let second_uri = self
                        .library_of(element)
                        .map(|l| ctx.library_uri(l))
                        .unwrap_or_default();
                    let uri = self.ast[directive].uri;
                    self.report_at(diag::ambiguous_export(&name, first_uri, second_uri), uri);
                    return;
                }
                _ => {
                    self.library_context.exported_elements.insert(name, element);
                }
            }
        }
    }

    /// Dart `_checkForAugmentationExtendsClauseAlreadyPresent`.
    fn check_for_augmentation_extends_clause_already_present(
        &mut self,
        node: Id<ClassDeclaration>,
        declared_fragment: FragmentId,
    ) {
        let n = &self.ast[node];
        if n.augment_keyword.is_none() {
            return;
        }
        let Some(extends_clause) = n.extends_clause else {
            return;
        };
        let ctx = self.ctx;
        let mut preceding = ctx
            .fragment_data(declared_fragment)
            .and_then(|f| f.previous_fragment);
        while let Some(p) = preceding {
            if self.fragment_has(p, FragmentFlags::CLASS_FRAGMENT_HAS_EXTENDS_CLAUSE) {
                let keyword = self.ast[extends_clause].extends_keyword;
                self.report_at_token(diag::augmentation_extends_clause_already_present(), keyword);
                break;
            }
            preceding = ctx.fragment_data(p).and_then(|f| f.previous_fragment);
        }
    }

    /// Dart `_checkForBadFunctionUse`.
    fn check_for_bad_function_use(
        &mut self,
        superclass: Option<Id<NamedType>>,
        implements_clause: Option<Id<ImplementsClause>>,
        with_clause: Option<Id<WithClause>>,
    ) {
        // With the `class_modifiers` feature `Function` is final.
        if self.feature_enabled(ExperimentalFlag::ClassModifiers) {
            return;
        }
        let ctx = self.ctx;
        let ast = self.ast;
        if let Some(superclass) = superclass
            && let Some(t) = self.named_type_type_opt(superclass)
            && ctx.is_dart_core_function(t)
        {
            self.report_at(diag::deprecated_extends_function(), superclass);
        }
        if let Some(implements_clause) = implements_clause {
            for &interface in ast.list(ast[implements_clause].interfaces) {
                if let Some(t) = self.named_type_type_opt(interface)
                    && ctx.is_dart_core_function(t)
                {
                    self.report_at(diag::deprecated_implements_function(), interface);
                    break;
                }
            }
        }
        if let Some(with_clause) = with_clause {
            for &mixin in ast.list(ast[with_clause].mixin_types) {
                if let Some(t) = self.named_type_type_opt(mixin)
                    && ctx.is_dart_core_function(t)
                {
                    self.report_at(diag::deprecated_mixin_function(), mixin);
                }
            }
        }
    }

    /// Dart `_checkForBaseClassOrMixinImplementedOutsideOfLibrary`.
    fn check_for_base_class_or_mixin_implemented_outside_of_library(
        &mut self,
        implements_clause: Option<Id<ImplementsClause>>,
    ) {
        let Some(implements_clause) = implements_clause else {
            return;
        };
        let ctx = self.ctx;
        let ast = self.ast;
        for &interface in ast.list(ast[implements_clause].interfaces) {
            let Some(element) = self.named_type_interface(interface) else {
                continue;
            };
            let mut implemented = vec![element];
            implemented.extend(
                ctx.element_all_supertypes(element)
                    .iter()
                    .filter_map(|&s| ctx.interface_element(s)),
            );
            for interface_element in implemented {
                let e = interface_element.raw();
                if self.is_base(e)
                    && self.library_of(e) != Some(self.current_library())
                    && !self.may_ignore_class_modifiers(self.library_of(e))
                {
                    let name = self.name_of(e);
                    if e.tag() == Tag::Class && !self.is_sealed(e) {
                        self.report_at(
                            diag::base_class_implemented_outside_of_library(&name),
                            interface,
                        );
                    } else if e.tag() == Tag::Mixin {
                        self.report_at(
                            diag::base_mixin_implemented_outside_of_library(&name),
                            interface,
                        );
                    }
                    break;
                }
            }
        }
    }

    /// Dart `_checkForClassAugmentationModifierMismatch`.
    fn check_for_class_augmentation_modifier_mismatch(
        &mut self,
        node: Id<ClassDeclaration>,
        declared_fragment: FragmentId,
    ) {
        let n = &self.ast[node];
        let Some(augment_keyword) = n.augment_keyword else {
            return;
        };
        let Some(element) = self.element_of(declared_fragment) else {
            return;
        };
        let first_fragment = self.first_fragment(element);
        if first_fragment == declared_fragment {
            return;
        }
        let checks = [
            (
                FragmentFlags::CLASS_FRAGMENT_IS_ABSTRACT,
                n.abstract_keyword,
                "abstract",
            ),
            (
                FragmentFlags::CLASS_FRAGMENT_IS_BASE,
                n.base_keyword,
                "base",
            ),
            (
                FragmentFlags::CLASS_FRAGMENT_IS_FINAL,
                n.final_keyword,
                "final",
            ),
            (
                FragmentFlags::CLASS_FRAGMENT_IS_INTERFACE,
                n.interface_keyword,
                "interface",
            ),
            (
                FragmentFlags::CLASS_FRAGMENT_IS_SEALED,
                n.sealed_keyword,
                "sealed",
            ),
            (
                FragmentFlags::CLASS_FRAGMENT_IS_MIXIN_CLASS,
                n.mixin_keyword,
                "mixin",
            ),
        ];
        for (flag, token, name) in checks {
            self.check_for_augmentation_modifier_mismatch(
                augment_keyword,
                self.fragment_has(declared_fragment, flag),
                self.fragment_has(first_fragment, flag),
                token,
                name,
            );
        }
    }

    /// Dart `_checkForClassUsedAsMixinDeclaresGenerativeConstructor`.
    fn check_for_class_used_as_mixin_declares_generative_constructor(
        &mut self,
        mixin_name: Id<NamedType>,
        mixin_element: EId<InterfaceElement>,
    ) -> bool {
        let ctx = self.ctx;
        for &constructor in &ctx.interface(mixin_element).constructors {
            let c = constructor.raw();
            if self.first_fragment_has(c, FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_ORIGIN_DECLARATION)
                && !self.first_fragment_has(c, FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY)
            {
                let name = self.name_of(mixin_element.raw());
                self.report_at(
                    diag::class_used_as_mixin_declares_generative_constructor(&name),
                    mixin_name,
                );
                return true;
            }
        }
        false
    }

    /// Dart `diagnosticReporter.report(diagnostic)` into the listener of
    /// the library analyzer, which keeps a set of diagnostics: an equal
    /// diagnostic (code, offset, length, message) is reported once. A
    /// declared field has a getter and a setter that report the same
    /// conflict.
    fn report_unique(&mut self, diagnostic: dartr_diagnostics::LocatedDiagnostic) {
        let d = diagnostic.into_diagnostic();
        let duplicate = self.diagnostics.iter().any(|e| {
            std::ptr::eq(e.code, d.code)
                && e.offset == d.offset
                && e.length == d.length
                && e.message == d.message
        });
        if !duplicate {
            self.diagnostics.push(d);
        }
    }

    /// Dart `_checkForConflictingClassMembers`.
    fn check_for_conflicting_class_members(&mut self, fragment: FragmentId) {
        let Some(enclosing_class) = self.enclosing_class else {
            return;
        };
        let ctx = self.ctx;
        let library = self.current_library();
        let class_name = self.name_of(enclosing_class.raw());
        let inheritance = self.inheritance;

        if enclosing_class.raw().tag() == Tag::ExtensionType {
            let interface = inheritance.get_interface(enclosing_class);
            for conflict in &interface.conflicts {
                match conflict {
                    Conflict::ExtensionTypeConflictingStaticAndInstance {
                        declared,
                        inherited,
                        ..
                    } => {
                        let declared_base = member::base_element(&ctx, *declared);
                        let first = ctx.element_data(declared_base).map(|d| d.first_fragment);
                        if first.and_then(|f| library_fragment_of(&ctx, f))
                            != Some(self.current_unit())
                        {
                            continue;
                        }
                        let member_name = self.name_of(declared_base);
                        let conflicting = member::enclosing_element(&ctx, *inherited)
                            .map(|e| self.name_of(e))
                            .unwrap_or_default();
                        let (offset, length) = self.diagnostic_range(declared_base);
                        self.report_unique(
                            diag::conflicting_static_and_instance(
                                &class_name,
                                &member_name,
                                &conflicting,
                            )
                            .at_offset(offset, length),
                        );
                    }
                    Conflict::ExtensionTypeConflictingInheritedMethodAndSetter { name, .. } => {
                        if self.fragment_has(fragment, FragmentFlags::FRAGMENT_IS_AUGMENTATION) {
                            continue;
                        }
                        let (offset, length) = self.diagnostic_range(enclosing_class.raw());
                        self.report_unique(
                            diag::conflicting_inherited_method_and_setter(
                                enclosing_class.raw().kind().display_name(),
                                &class_name,
                                name.text(&ctx),
                            )
                            .at_offset(offset, length),
                        );
                    }
                    _ => {}
                }
            }
            return;
        }

        let Some(instance) = self.instance_fragment(fragment) else {
            return;
        };
        let methods = instance.methods.clone();
        let accessors: Vec<FragmentId> = instance
            .getters
            .iter()
            .map(|g| g.raw())
            .chain(instance.setters.iter().map(|s| s.raw()))
            .collect();
        let mut conflicting_declared_names: IndexSet<String> = IndexSet::new();

        // method declared in the enclosing class vs. inherited getter/setter
        for method in methods {
            let name = ctx
                .fragment(method)
                .name
                .map(|n| ctx.name_str(n).to_string())
                .unwrap_or_default();
            let getter =
                inheritance.get_inherited(enclosing_class, Name::new(&ctx, Some(library), &name));
            let setter = inheritance.get_inherited(
                enclosing_class,
                Name::new(&ctx, Some(library), &format!("{name}=")),
            );
            let range = ctx
                .fragment(method)
                .element
                .try_get()
                .map(|&e| self.diagnostic_range(e))
                .unwrap_or((0, 0));

            if self.fragment_has(method.raw(), FragmentFlags::EXECUTABLE_FRAGMENT_IS_STATIC)
                && let Some(inherited) = getter.or(setter)
            {
                let conflicting = member::enclosing_element(&ctx, inherited)
                    .map(|e| self.name_of(e))
                    .unwrap_or_default();
                self.report_unique(
                    diag::conflicting_static_and_instance(&class_name, &name, &conflicting)
                        .at_offset(range.0, range.1),
                );
                continue;
            }

            let field_conflict = match (getter, setter) {
                (Some(g), _) if member::is_getter(&ctx, g) => Some(g),
                (_, Some(s)) if member::is_setter(&ctx, s) => Some(s),
                _ => None,
            };
            if let Some(inherited) = field_conflict {
                let conflicting = member::enclosing_element(&ctx, inherited)
                    .map(|e| self.name_of(e))
                    .unwrap_or_default();
                self.report_unique(
                    diag::conflicting_method_and_field(&class_name, &name, &conflicting)
                        .at_offset(range.0, range.1),
                );
            }
        }

        // getter declared in the enclosing class vs. inherited method
        for accessor in accessors {
            let Some(data) = ctx.fragment_data(accessor) else {
                continue;
            };
            let name = data
                .name
                .map(|n| ctx.name_str(n))
                .unwrap_or_default()
                .trim_end_matches('=')
                .to_string();
            let inherited = inheritance
                .get_inherited(enclosing_class, Name::new(&ctx, Some(library), &name))
                .or_else(|| {
                    inheritance.get_inherited(
                        enclosing_class,
                        Name::new(&ctx, Some(library), &format!("{name}=")),
                    )
                });
            let range = data
                .element
                .try_get()
                .map(|&e| self.diagnostic_range(e))
                .unwrap_or((0, 0));
            let is_static =
                self.fragment_has(accessor, FragmentFlags::EXECUTABLE_FRAGMENT_IS_STATIC);
            match inherited {
                Some(inherited) if is_static => {
                    let conflicting = member::enclosing_element(&ctx, inherited)
                        .map(|e| self.name_of(e))
                        .unwrap_or_default();
                    self.report_unique(
                        diag::conflicting_static_and_instance(&class_name, &name, &conflicting)
                            .at_offset(range.0, range.1),
                    );
                    conflicting_declared_names.insert(name);
                }
                Some(inherited) if member::is_method(&ctx, inherited) => {
                    let conflicting = member::enclosing_element(&ctx, inherited)
                        .map(|e| self.name_of(e))
                        .unwrap_or_default();
                    self.report_unique(
                        diag::conflicting_field_and_method(&class_name, &name, &conflicting)
                            .at_offset(range.0, range.1),
                    );
                    conflicting_declared_names.insert(name);
                }
                _ => {}
            }
        }

        // Inherited method and setter with the same name.
        let inherited = inheritance.get_inherited_map(enclosing_class);
        for (method_name, &method) in inherited {
            if !member::is_method(&ctx, method) {
                continue;
            }
            if conflicting_declared_names.contains(method_name.text(&ctx)) {
                continue;
            }
            let setter_name = method_name.for_setter(&ctx);
            if let Some(&setter) = inherited.get(&setter_name)
                && member::is_setter(&ctx, setter)
            {
                let (offset, length) = self.diagnostic_range(enclosing_class.raw());
                self.report_unique(
                    diag::conflicting_inherited_method_and_setter(
                        enclosing_class.raw().kind().display_name(),
                        &class_name,
                        method_name.text(&ctx),
                    )
                    .at_offset(offset, length),
                );
            }
        }
    }

    /// Dart `_checkForConflictingClassTypeVariableErrorCodes`.
    fn check_for_conflicting_class_type_variable_error_codes(&mut self) {
        let Some(enclosing_class) = self.enclosing_class else {
            return;
        };
        let ctx = self.ctx;
        let is_mixin = enclosing_class.raw().tag() == Tag::Mixin;
        let class_name = ctx.element_name(enclosing_class.raw());
        for &type_parameter in &ctx.instance(enclosing_class.upcast()).type_params {
            if self.is_wildcard_variable(type_parameter) {
                continue;
            }
            let Some(name) = ctx.element_name(type_parameter.raw()) else {
                continue;
            };
            let (offset, length) = self.diagnostic_range(type_parameter.raw());
            // name is same as the name of the enclosing class
            if class_name == Some(name) {
                let d = if is_mixin {
                    diag::conflicting_type_variable_and_mixin(name)
                } else {
                    diag::conflicting_type_variable_and_class(name)
                };
                self.report(d.at_offset(offset, length));
            }
            // check members
            let instance = enclosing_class.upcast();
            if lookup::get_named_constructor(&ctx, enclosing_class, name).is_some()
                || lookup::get_method(&ctx, instance, name).is_some()
                || lookup::get_getter(&ctx, instance, name).is_some()
                || lookup::get_setter(&ctx, instance, name).is_some()
            {
                let d = if is_mixin {
                    diag::conflicting_type_variable_and_member_mixin(name)
                } else {
                    diag::conflicting_type_variable_and_member_class(name)
                };
                self.report(d.at_offset(offset, length));
            }
        }
    }

    /// Dart `_checkForConflictingEnumTypeVariableErrorCodes`.
    fn check_for_conflicting_enum_type_variable_error_codes(&mut self, fragment: FragmentId) {
        let ctx = self.ctx;
        let Some(element) = self.element_of(fragment) else {
            return;
        };
        let fragment_name = ctx
            .fragment_data(fragment)
            .and_then(|f| f.name)
            .map(|n| ctx.name_str(n));
        for type_parameter in self.fragment_type_parameters(fragment) {
            let Some(tp) = self.element_of(type_parameter) else {
                continue;
            };
            let name = ctx.element_name(tp).unwrap_or("");
            let (offset, length) = self.diagnostic_range(tp);
            if fragment_name == Some(name) {
                self.report(
                    diag::conflicting_type_variable_and_enum(name).at_offset(offset, length),
                );
            }
            let instance = EId::from_raw(element);
            if lookup::get_method(&ctx, instance, name).is_some()
                || lookup::get_getter(&ctx, instance, name).is_some()
                || lookup::get_setter(&ctx, instance, name).is_some()
            {
                self.report(
                    diag::conflicting_type_variable_and_member_enum(name).at_offset(offset, length),
                );
            }
        }
    }

    /// Dart `_checkForConflictingExtensionTypeTypeVariableErrorCodes`.
    fn check_for_conflicting_extension_type_type_variable_error_codes(
        &mut self,
        fragment: FragmentId,
    ) {
        let ctx = self.ctx;
        let Some(element) = self.element_of(fragment) else {
            return;
        };
        let fragment_name = ctx
            .fragment_data(fragment)
            .and_then(|f| f.name)
            .map(|n| ctx.name_str(n));
        for type_parameter in self.fragment_type_parameters(fragment) {
            let Some(tp) = self.element_of(type_parameter) else {
                continue;
            };
            if self.is_wildcard_variable(EId::from_raw(tp)) {
                continue;
            }
            let name = ctx.element_name(tp).unwrap_or("");
            let (offset, length) = self.diagnostic_range(tp);
            if fragment_name == Some(name) {
                self.report(
                    diag::conflicting_type_variable_and_extension_type(name)
                        .at_offset(offset, length),
                );
            }
            let instance = EId::from_raw(element);
            if lookup::get_named_constructor(&ctx, EId::from_raw(element), name).is_some()
                || lookup::get_method(&ctx, instance, name).is_some()
                || lookup::get_getter(&ctx, instance, name).is_some()
                || lookup::get_setter(&ctx, instance, name).is_some()
            {
                self.report(
                    diag::conflicting_type_variable_and_member_extension_type(name)
                        .at_offset(offset, length),
                );
            }
        }
    }

    /// Dart `_checkForConflictingExtensionTypeVariableErrorCodes`.
    fn check_for_conflicting_extension_type_variable_error_codes(&mut self) {
        let Some(extension) = self.enclosing_extension else {
            return;
        };
        let ctx = self.ctx;
        let extension_name = ctx.element_name(extension.raw());
        for &type_parameter in &ctx.get(extension).type_params {
            let Some(name) = ctx.element_name(type_parameter.raw()) else {
                continue;
            };
            let (offset, length) = self.diagnostic_range(type_parameter.raw());
            if extension_name == Some(name) {
                self.report(
                    diag::conflicting_type_variable_and_extension(name).at_offset(offset, length),
                );
            }
            let instance = extension.upcast();
            if lookup::get_method(&ctx, instance, name).is_some()
                || lookup::get_getter(&ctx, instance, name).is_some()
                || lookup::get_setter(&ctx, instance, name).is_some()
            {
                self.report(
                    diag::conflicting_type_variable_and_member_extension(name)
                        .at_offset(offset, length),
                );
            }
        }
    }

    /// Dart `_checkForConflictingGenerics(node:, nameToken:)`.
    fn check_for_conflicting_generics(&mut self, fragment: FragmentId, name_token: TokenId) {
        // Report only on the declaration.
        if self.fragment_has(fragment, FragmentFlags::FRAGMENT_IS_AUGMENTATION) {
            return;
        }
        let Some(element) = self
            .element_of(fragment)
            .and_then(|e| e.cast::<InterfaceElement>())
        else {
            return;
        };
        let Some(enclosing_class) = self.enclosing_class else {
            return;
        };
        let ctx = self.ctx;
        for error in dartr_typesystem::class_hierarchy::errors(&ctx, element) {
            let name = self.name_of(enclosing_class.raw());
            self.report_at_token(
                diag::conflicting_generic_interfaces(
                    enclosing_class.raw().kind().display_name(),
                    &name,
                    &type_display_string(&ctx, error.first, false),
                    &type_display_string(&ctx, error.second, false),
                ),
                name_token,
            );
        }
    }

    /// Dart `_checkForDeferredImportOfExtensions`.
    fn check_for_deferred_import_of_extensions(
        &mut self,
        directive: Id<ImportDirective>,
        combinators: &[NamespaceCombinator],
        imported_library: Option<EId<LibraryElement>>,
    ) {
        let Some(imported_library) = imported_library else {
            return;
        };
        let namespace = self.export_namespace_for_directive(imported_library, combinators);
        if namespace.values().any(|e| e.tag() == Tag::Extension) {
            let uri = self.ast[directive].uri;
            self.report_at(diag::deferred_import_of_extension(), uri);
        }
    }

    /// Dart `_checkForDeferredPrefixCollisions`.
    fn check_for_deferred_prefix_collisions(&mut self, unit: Id<CompilationUnit>) {
        let ast = self.ast;
        let mut prefix_to_directives: IndexMap<ElementId, Vec<Id<ImportDirective>>> =
            IndexMap::new();
        for &directive in ast.list(ast[unit].directives) {
            let Some(import) = ast.cast::<ImportDirective>(directive.raw()) else {
                continue;
            };
            let Some(prefix) = ast[import].prefix else {
                continue;
            };
            if let Some(ElemRef::Base(element)) = self.element(prefix)
                && element.tag() == Tag::Prefix
            {
                prefix_to_directives
                    .entry(element)
                    .or_default()
                    .push(import);
            }
        }
        for imports in prefix_to_directives.values() {
            self.check_deferred_prefix_collision(imports);
        }
    }

    /// Dart `_checkForEnumWithNameValues`.
    fn check_for_enum_with_name_values(&mut self, type_name: TokenId) {
        if self.ast.tokens.lexeme(type_name) == "values" {
            self.report_at_token(diag::enum_with_name_values(), type_name);
        }
    }

    /// Dart `_checkForExportInternalLibrary`.
    fn check_for_export_internal_library(
        &mut self,
        directive: Id<ExportDirective>,
        exported_library: Option<EId<LibraryElement>>,
    ) {
        if self.is_in_system_library {
            return;
        }
        let Some(exported_library) = exported_library else {
            return;
        };
        // should be private
        if !self.ctx.library_uri(exported_library).starts_with("dart:_") {
            return;
        }
        let uri = self.string_literal_value(self.ast[directive].uri.raw());
        self.report_at(diag::export_internal_library(&uri), directive);
    }

    /// Dart `_checkForExtendsDeferredClass`.
    fn check_for_extends_deferred_class(&mut self, superclass: Option<Id<NamedType>>) {
        if let Some(superclass) = superclass {
            self.check_for_extends_or_implements_deferred_class(
                superclass,
                diag::extends_deferred_class(),
            );
        }
    }

    /// Dart `_checkForExtendsDisallowedClass`.
    fn check_for_extends_disallowed_class(&mut self, superclass: Option<Id<NamedType>>) -> bool {
        match superclass {
            Some(superclass) => self.check_for_extends_or_implements_disallowed_class(superclass),
            None => false,
        }
    }

    /// Dart `_checkForExtendsOrImplementsDeferredClass`.
    fn check_for_extends_or_implements_deferred_class(
        &mut self,
        named_type: Id<NamedType>,
        locatable_diagnostic: LocatableDiagnostic,
    ) -> bool {
        if self.named_type_is_synthetic(named_type) {
            return false;
        }
        if self.named_type_is_deferred(named_type) {
            self.report_at(locatable_diagnostic, named_type);
            return true;
        }
        false
    }

    /// Dart `_checkForExtendsOrImplementsDisallowedClass`.
    fn check_for_extends_or_implements_disallowed_class(&self, named_type: Id<NamedType>) -> bool {
        if self.named_type_is_synthetic(named_type) {
            return false;
        }
        // The SDK implementation may implement disallowed types. For example,
        // JSNumber in dart2js and _Smi in Dart VM both implement int.
        if self.is_in_system_library {
            return false;
        }
        let ctx = self.ctx;
        let Some(element) = self.named_type_interface(named_type) else {
            return false;
        };
        let name = self.name_of(element.raw());
        let uri = self
            .library_of(element.raw())
            .map(|l| ctx.library_uri(l))
            .unwrap_or_default();
        TypeProvider::is_non_subtypable_class(&name, uri)
    }

    /// Dart `_checkForExtensionTypeImplementsDeferred`.
    fn check_for_extension_type_implements_deferred(&mut self, node: Id<ExtensionTypeDeclaration>) {
        let ast = self.ast;
        let Some(clause) = ast[node].implements_clause else {
            return;
        };
        for &t in ast.list(ast[clause].interfaces) {
            self.check_for_extends_or_implements_deferred_class(
                t,
                diag::implements_deferred_class(),
            );
        }
    }

    /// Dart `_checkForExtensionTypeImplementsItself`.
    fn check_for_extension_type_implements_itself(
        &mut self,
        node: Id<ExtensionTypeDeclaration>,
        element: EId<ExtensionTypeElement>,
    ) {
        if self.ctx.get(element).has_implements_self_reference.get() {
            let name = name_part_type_name(self.ast, self.ast[node].name_part);
            self.report_at_token(diag::extension_type_implements_itself(), name);
        }
    }

    /// Dart `_checkForExtensionTypeMemberConflicts`.
    fn check_for_extension_type_member_conflicts(
        &mut self,
        node: Id<ExtensionTypeDeclaration>,
        element: EId<ExtensionTypeElement>,
    ) {
        let ctx = self.ctx;
        let type_name = name_part_type_name(self.ast, self.ast[node].name_part);
        let type_name_text = self.ast.tokens.lexeme(type_name).to_string();
        let interface = self.inheritance.get_interface(element.upcast());
        for conflict in &interface.conflicts {
            match conflict {
                Conflict::Candidates { name, .. }
                | Conflict::HasNonExtensionAndExtensionMember { name, .. }
                | Conflict::NotUniqueExtensionMember { name, .. } => {
                    self.report_at_token(
                        diag::extension_type_inherited_member_conflict(
                            &type_name_text,
                            name.text(&ctx),
                        ),
                        type_name,
                    );
                }
                _ => {}
            }
        }
    }

    /// Dart `_checkForExtensionTypeRepresentationDependsOnItself`.
    fn check_for_extension_type_representation_depends_on_itself(
        &mut self,
        node: Id<ExtensionTypeDeclaration>,
        element: EId<ExtensionTypeElement>,
    ) {
        if self
            .ctx
            .get(element)
            .has_representation_self_reference
            .get()
        {
            let name = name_part_type_name(self.ast, self.ast[node].name_part);
            self.report_at_token(
                diag::extension_type_representation_depends_on_itself(),
                name,
            );
        }
    }

    /// Dart `_checkForExtensionTypeRepresentationErrorCodes`.
    fn check_for_extension_type_representation_error_codes(
        &mut self,
        node: Id<ExtensionTypeDeclaration>,
    ) {
        let ast = self.ast;
        let tokens = &ast.tokens;
        let Some(primary_constructor) =
            ast.cast::<PrimaryConstructorDeclaration>(ast[node].name_part.raw())
        else {
            return;
        };
        let formal_parameter_list = ast[primary_constructor].formal_parameters;
        let list = &ast[formal_parameter_list];
        let formal_parameters = ast.list(list.parameters);

        let Some(&first) = formal_parameters.first() else {
            let next = tokens.next(list.left_parenthesis);
            self.report_at_token(diag::expected_representation_field(), next);
            return;
        };
        let inner = first.raw();

        if formal_parameters.len() > 1 {
            let next = tokens.next(ast.end_token(inner));
            self.report_at_token(diag::multiple_representation_fields(), next);
            return;
        }

        if let Some(p) = ast.cast::<FieldFormalParameter>(inner) {
            self.report_at_token(diag::expected_representation_field(), ast[p].this_keyword);
            return;
        }
        if let Some(p) = ast.cast::<SuperFormalParameter>(inner) {
            self.report_at_token(diag::expected_representation_field(), ast[p].super_keyword);
            return;
        }
        let regular = ast.cast::<RegularFormalParameter>(inner);
        let Some(name_token) = regular.and_then(|r| ast[r].name) else {
            self.report_at(diag::expected_representation_field(), inner);
            return;
        };

        if tokens.lexeme(name_token) == tokens.lexeme(ast[primary_constructor].type_name) {
            self.report_at_token(diag::member_with_class_name(), name_token);
        }

        let ctx = self.ctx;
        if ctx.tp.is_object_member(&ctx, tokens.lexeme(name_token)) {
            self.report_at_token(diag::extension_type_declares_member_of_object(), name_token);
        }

        if self.feature_enabled(ExperimentalFlag::PrimaryConstructors) {
            if let Some(r) = regular
                && let Some(keyword) = ast[r].const_final_or_var_keyword
                && tokens.ty(keyword) == Keyword::VAR
            {
                self.report_at_token(diag::representation_field_modifier(), keyword);
            }
        } else {
            if let Some(left_delimiter) = list.left_delimiter {
                self.report_at_token(diag::expected_representation_field(), left_delimiter);
                return;
            }
            if let Some(r) = regular {
                let r = &ast[r];
                if r.function_typed_suffix.is_some() {
                    self.report_at_token(
                        diag::expected_representation_field(),
                        ast.begin_token(inner),
                    );
                    return;
                }
                if let Some(keyword) = r.const_final_or_var_keyword
                    && matches!(tokens.ty(keyword), Keyword::FINAL | Keyword::VAR)
                {
                    self.report_at_token(diag::representation_field_modifier(), keyword);
                }
                if r.type_.is_none() {
                    self.report_at_token(diag::expected_representation_type(), name_token);
                }
            }
            let maybe_comma = tokens.next(ast.end_token(inner));
            if tokens.ty(maybe_comma) == TokenType::COMMA {
                self.report_at_token(diag::representation_field_trailing_comma(), maybe_comma);
            }
        }
    }

    /// Dart `_checkForExtensionTypeRepresentationTypeBottom`.
    fn check_for_extension_type_representation_type_bottom(
        &mut self,
        node: Id<ExtensionTypeDeclaration>,
        element: EId<ExtensionTypeElement>,
    ) {
        let ctx = self.ctx;
        if ctx.get(element).fields.is_empty() {
            return;
        }
        let representation_type = ctx.extension_type_representation(element);
        if ctx.is_bottom(representation_type)
            && let Some(type_node) = self.representation_formal_parameter_type(node)
        {
            self.report_at(diag::extension_type_representation_type_bottom(), type_node);
        }
    }

    /// Dart `_checkForExtensionTypeWithAbstractMember`.
    fn check_for_extension_type_with_abstract_member(
        &mut self,
        node: Id<ExtensionTypeDeclaration>,
        members: &[Id<ClassMember>],
    ) {
        if self.feature_enabled(ExperimentalFlag::Augmentations) {
            return;
        }
        let ast = self.ast;
        let type_name = ast
            .tokens
            .lexeme(name_part_type_name(ast, ast[node].name_part));
        for &member in members {
            let Some(method) = ast.cast::<MethodDeclaration>(member.raw()) else {
                continue;
            };
            let m = &ast[method];
            let is_static = m
                .modifier_keyword
                .is_some_and(|k| ast.tokens.lexeme(k) == "static");
            if is_static {
                continue;
            }
            let is_complete =
                m.external_keyword.is_some() || !ast.is::<EmptyFunctionBody>(m.body.raw());
            if !is_complete {
                self.report_at(
                    diag::extension_type_with_abstract_member(ast.tokens.lexeme(m.name), type_name),
                    method,
                );
            }
        }
    }

    /// Dart `_checkForFinalSupertypeOutsideOfLibrary` (the `withClause`
    /// argument is not read by the Dart code).
    fn check_for_final_supertype_outside_of_library(
        &mut self,
        superclass: Option<Id<NamedType>>,
        implements_clause: Option<Id<ImplementsClause>>,
        on_clause: Option<Id<MixinOnClause>>,
    ) {
        let ctx = self.ctx;
        let ast = self.ast;
        if let Some(superclass) = superclass
            && let Some(element) = self.named_type_interface(superclass)
            && self.is_final_class_outside(element.raw())
        {
            let name = self.name_of(element.raw());
            self.report_at(
                diag::final_class_extended_outside_of_library(&name),
                superclass,
            );
        }
        if let Some(implements_clause) = implements_clause {
            for &named_type in ast.list(ast[implements_clause].interfaces) {
                let Some(type_element) = self.named_type_interface(named_type) else {
                    continue;
                };
                let mut implemented = vec![type_element];
                implemented.extend(
                    ctx.element_all_supertypes(type_element)
                        .iter()
                        .filter_map(|&s| ctx.interface_element(s)),
                );
                for element in implemented {
                    if self.is_final_class_outside(element.raw()) {
                        // If the final interface is an indirect interface and
                        // is in a different library that has class modifiers
                        // enabled, there is a nearer declaration that would
                        // emit an error, if any.
                        if element != type_element
                            && self.library_of(type_element.raw()).is_some_and(|l| {
                                crate::scope::library_feature_enabled(
                                    &ctx,
                                    l,
                                    ExperimentalFlag::ClassModifiers,
                                )
                            })
                        {
                            continue;
                        }
                        let name = self.name_of(element.raw());
                        self.report_at(
                            diag::final_class_implemented_outside_of_library(&name),
                            named_type,
                        );
                        break;
                    }
                }
            }
        }
        if let Some(on_clause) = on_clause {
            for &named_type in ast.list(ast[on_clause].superclass_constraints) {
                if let Some(element) = self.named_type_interface(named_type)
                    && self.is_final_class_outside(element.raw())
                {
                    let name = self.name_of(element.raw());
                    self.report_at(
                        diag::final_class_used_as_mixin_constraint_outside_of_library(&name),
                        named_type,
                    );
                }
            }
        }
    }

    /// The condition of `_checkForFinalSupertypeOutsideOfLibrary`: a final,
    /// not sealed class of another library whose modifiers apply.
    fn is_final_class_outside(&self, e: ElementId) -> bool {
        e.tag() == Tag::Class
            && self.element_has(e, ElementFlags::CLASS_ELEMENT_IS_FINAL)
            && !self.is_sealed(e)
            && self.library_of(e) != Some(self.current_library())
            && !self.may_ignore_class_modifiers(self.library_of(e))
    }

    /// Dart `_checkForImplementsClauseErrorCodes`.
    fn check_for_implements_clause_error_codes(
        &mut self,
        clause: Option<Id<ImplementsClause>>,
    ) -> bool {
        let Some(clause) = clause else {
            return false;
        };
        let ast = self.ast;
        let mut found_error = false;
        for &t in ast.list(ast[clause].interfaces) {
            if self.check_for_extends_or_implements_disallowed_class(t)
                || self.check_for_extends_or_implements_deferred_class(
                    t,
                    diag::implements_deferred_class(),
                )
            {
                found_error = true;
            }
        }
        found_error
    }

    /// Dart `_checkForImportInternalLibrary`.
    fn check_for_import_internal_library(
        &mut self,
        directive: Id<ImportDirective>,
        imported_library: Option<EId<LibraryElement>>,
    ) {
        if self.is_in_system_library || self.is_wasm(imported_library) {
            return;
        }
        let Some(imported_library) = imported_library else {
            return;
        };
        // should be private
        if !self.ctx.library_uri(imported_library).starts_with("dart:_") {
            return;
        }
        let uri = self.ast[directive].uri;
        let value = self.string_literal_value(uri.raw());
        self.report_at(diag::import_internal_library(&value), uri);
    }

    /// Dart `_checkForInterfaceClassOrMixinSuperclassOutsideOfLibrary` (the
    /// `withClause` argument is not read by the Dart code).
    fn check_for_interface_class_or_mixin_superclass_outside_of_library(
        &mut self,
        superclass: Option<Id<NamedType>>,
    ) {
        let Some(superclass) = superclass else {
            return;
        };
        let Some(element) = self.named_type_interface(superclass) else {
            return;
        };
        let e = element.raw();
        if e.tag() == Tag::Class
            && self.element_has(e, ElementFlags::CLASS_ELEMENT_IS_INTERFACE)
            && !self.is_sealed(e)
            && self.library_of(e) != Some(self.current_library())
            && !self.may_ignore_class_modifiers(self.library_of(e))
        {
            let name = self.name_of(e);
            self.report_at(
                diag::interface_class_extended_outside_of_library(&name),
                superclass,
            );
        }
    }

    /// Dart `_checkForMixinAugmentationModifierMismatch`.
    fn check_for_mixin_augmentation_modifier_mismatch(
        &mut self,
        node: Id<MixinDeclaration>,
        declared_fragment: FragmentId,
    ) {
        let n = &self.ast[node];
        let Some(augment_keyword) = n.augment_keyword else {
            return;
        };
        let Some(element) = self.element_of(declared_fragment) else {
            return;
        };
        let first_fragment = self.first_fragment(element);
        if first_fragment == declared_fragment {
            return;
        }
        self.check_for_augmentation_modifier_mismatch(
            augment_keyword,
            self.fragment_has(declared_fragment, FragmentFlags::MIXIN_FRAGMENT_IS_BASE),
            self.fragment_has(first_fragment, FragmentFlags::MIXIN_FRAGMENT_IS_BASE),
            n.base_keyword,
            "base",
        );
    }

    /// Dart `_checkForMixinClassErrorCodes`.
    fn check_for_mixin_class_error_codes(
        &mut self,
        node: NodeId,
        members: &[Id<ClassMember>],
        superclass: Option<Id<NamedType>>,
        with_clause: Option<Id<WithClause>>,
    ) {
        let ast = self.ast;
        let tokens = &ast.tokens;
        let Some(element) = self
            .declared_fragment(node)
            .and_then(|f| self.element_of(f))
        else {
            return;
        };
        if !self.is_mixin_class(element) {
            return;
        }
        let Some(class_name) = self.ctx.element_name(element) else {
            return;
        };

        // Check that the class does not have a constructor.
        for &member in members {
            let Some(constructor) = ast.cast::<ConstructorDeclaration>(member.raw()) else {
                continue;
            };
            let c = &ast[constructor];
            if c.factory_keyword.is_some() {
                continue;
            }
            // Dart `ConstructorDeclaration.isTrivial`.
            let is_trivial = c.redirected_constructor.is_none()
                && ast.list(ast[c.parameters].parameters).is_empty()
                && ast.list(c.initializers).is_empty()
                && ast.is::<EmptyFunctionBody>(c.body.raw())
                && c.external_keyword.is_none();
            if is_trivial {
                continue;
            }
            // Dart `ConstructorDeclaration.errorRange`.
            let (start, start_end) = match (c.type_name, c.new_keyword.or(c.factory_keyword)) {
                (Some(t), _) => (ast.offset(t), ast.end(t)),
                (None, Some(k)) => (tokens.offset(k), tokens.get(k).end()),
                (None, None) => continue,
            };
            let end = c.name.map(|n| tokens.get(n).end()).unwrap_or(start_end);
            self.report(
                diag::mixin_class_declares_non_trivial_generative_constructor(class_name)
                    .at_offset(start as usize, end.saturating_sub(start) as usize),
            );
        }
        if let Some(class) = ast.cast::<ClassDeclaration>(node)
            && let Some(primary) =
                ast.cast::<PrimaryConstructorDeclaration>(ast[class].name_part.raw())
        {
            let p = &ast[primary];
            if !ast.list(ast[p.formal_parameters].parameters).is_empty() {
                // Dart `PrimaryConstructorDeclaration.errorRange`.
                let begin = ast.begin_token(primary);
                let start = tokens.offset(begin);
                let end = match p.constructor_name {
                    Some(n) => ast.end(n),
                    None => tokens.get(begin).end(),
                };
                self.report(
                    diag::mixin_class_declares_non_trivial_generative_constructor(class_name)
                        .at_offset(start as usize, end.saturating_sub(start) as usize),
                );
            } else if let Some(body) = members
                .iter()
                .find_map(|m| ast.cast::<PrimaryConstructorBody>(m.raw()))
            {
                // Dart `primaryConstructor.body`.
                let b = &ast[body];
                if !ast.list(b.initializers).is_empty() {
                    if let Some(colon) = b.colon {
                        self.report_at_token(
                            diag::mixin_class_declares_non_trivial_generative_constructor(
                                class_name,
                            ),
                            colon,
                        );
                    }
                } else if let Some(block_body) = ast.cast::<BlockFunctionBody>(b.body.raw()) {
                    let left_bracket = ast[ast[block_body].block].left_bracket;
                    self.report_at_token(
                        diag::mixin_class_declares_non_trivial_generative_constructor(class_name),
                        left_bracket,
                    );
                }
            }
        }
        // Check that the class has 'Object' as their superclass.
        if let Some(superclass) = superclass
            && !self
                .ctx
                .is_dart_core_object(self.named_type_type(superclass))
        {
            self.report_at(
                diag::mixin_class_declaration_extends_not_object(class_name),
                superclass,
            );
        } else if let Some(with_clause) = with_clause {
            if !self.is_mixin_application(element) {
                self.report_at(
                    diag::mixin_class_declaration_with_clause(class_name),
                    with_clause,
                );
            } else if ast.list(ast[with_clause].mixin_types).len() >= 2 {
                self.report_at(
                    diag::mixin_modifier_mixin_application_class_with_multiple_mixins(class_name),
                    with_clause,
                );
            }
        }
    }

    /// Dart `_checkForMixinInheritsNotFromObject`.
    fn check_for_mixin_inherits_not_from_object(
        &mut self,
        mixin_name: Id<NamedType>,
        mixin_element: EId<InterfaceElement>,
    ) -> bool {
        let e = mixin_element.raw();
        if e.tag() != Tag::Class {
            return false;
        }
        let ctx = self.ctx;
        let mixin_supertype = ctx.element_supertype(mixin_element);
        if mixin_supertype.is_none_or(|s| ctx.is_dart_core_object(s)) {
            let mixins = ctx.element_mixins(mixin_element);
            if mixins.is_empty() || (self.is_mixin_application(e) && mixins.len() < 2) {
                return false;
            }
        }
        let name = self.name_of(e);
        self.report_at(diag::mixin_inherits_from_not_object(&name), mixin_name);
        true
    }

    /// Dart `_checkForMixinSuperclassConstraints`.
    fn check_for_mixin_superclass_constraints(
        &mut self,
        mixin_index: usize,
        mixin_name: Id<NamedType>,
    ) -> bool {
        let Some(enclosing_class) = self.enclosing_class else {
            return false;
        };
        let ctx = self.ctx;
        let ts = self.type_system;
        let mixin_type = self.named_type_type(mixin_name);
        let Some(super_type) = ctx.element_supertype(enclosing_class) else {
            return false;
        };
        let super_type = ctx.with_nullability(super_type, dartr_element::Nullability::None);
        let mixins = ctx.element_mixins(enclosing_class);
        for constraint in ctx.superclass_constraints(mixin_type) {
            let mut is_satisfied = ts.is_subtype_of(super_type, constraint);
            if !is_satisfied {
                for i in 0..mixin_index {
                    if is_satisfied {
                        break;
                    }
                    // If there are less mixin types than mixin nodes, escape.
                    let Some(&m) = mixins.get(i) else {
                        return false;
                    };
                    // Probe a previous mixin type.
                    is_satisfied = ts.is_subtype_of(m, constraint);
                }
            }
            if !is_satisfied {
                let name = self.ast[mixin_name].name;
                self.report_at_token(
                    diag::mixin_application_not_implemented_interface(
                        type_arg(&ctx, mixin_type),
                        type_arg(&ctx, super_type),
                        type_arg(&ctx, constraint),
                    ),
                    name,
                );
                return true;
            }
        }
        false
    }

    /// Dart `_checkForMixinSuperInvokedMembers`.
    fn check_for_mixin_super_invoked_members(
        &mut self,
        mixin_index: i32,
        mixin_name: Id<NamedType>,
        mixin_element: EId<InterfaceElement>,
        mixin_type: TypeId,
    ) -> bool {
        let Some(enclosing_class) = self.enclosing_class else {
            return false;
        };
        let ctx = self.ctx;
        let first: FId<MixinFragment> = FId::from_raw(self.first_fragment(mixin_element.raw()));
        let Some(names) = ctx.fragment(first).super_invoked_names.try_get() else {
            return false;
        };
        let mixin_library = self.library_of(mixin_element.raw());
        let inheritance = self.inheritance;
        for &name in names {
            let name = ctx.name_str(name);
            let name_object = Name::new(&ctx, mixin_library, name);
            let super_member = inheritance.get_member_with(
                enclosing_class,
                name_object,
                GetMemberOptions {
                    concrete: true,
                    for_mixin_index: mixin_index,
                    for_super: true,
                },
            );
            let Some(super_member) = super_member else {
                let d = match name.strip_suffix('=') {
                    Some(setter) => {
                        diag::mixin_application_no_concrete_super_invoked_setter(setter)
                    }
                    None => diag::mixin_application_no_concrete_super_invoked_member(name),
                };
                self.report_at(d, mixin_name);
                return true;
            };
            let mixin_member = inheritance.get_member3(
                mixin_type,
                name_object,
                GetMemberOptions {
                    for_super: true,
                    ..GetMemberOptions::default()
                },
            );
            if let Some(mixin_member) = mixin_member
                && !correct_override::is_correct_override_of(
                    &self.type_system,
                    super_member,
                    mixin_member,
                )
            {
                self.report_at(
                    diag::mixin_application_concrete_super_invoked_member_type(
                        name,
                        type_arg(&ctx, member::type_(&ctx, mixin_member)),
                        type_arg(&ctx, member::type_(&ctx, super_member)),
                    ),
                    mixin_name,
                );
                return true;
            }
        }
        false
    }

    /// Dart `_checkForMixinWithConflictingPrivateMember`.
    fn check_for_mixin_with_conflicting_private_member(
        &mut self,
        with_clause: Option<Id<WithClause>>,
        superclass_name: Option<Id<NamedType>>,
    ) {
        let Some(with_clause) = with_clause else {
            return;
        };
        let ctx = self.ctx;
        let ast = self.ast;
        let declared_supertype = match superclass_name {
            Some(s) => self.named_type_type(s),
            None => ctx.tp.object_type(),
        };
        let Some(declared_super_element) = ctx.interface_element(declared_supertype) else {
            return;
        };
        let inheritance = self.inheritance;
        let mut mixed_in_names: IndexMap<EId<LibraryElement>, IndexMap<String, String>> =
            IndexMap::new();
        for &mixin_type in ast.list(ast[with_clause].mixin_types) {
            let t = self.named_type_type(mixin_type);
            let Some(element) = ctx.interface_element(t) else {
                continue;
            };
            let Some(library) = self.library_of(element.raw()) else {
                continue;
            };
            if library == self.current_library() {
                continue;
            }
            let mixin_lexeme = ast.tokens.lexeme(ast[mixin_type].name).to_string();
            let instance = ctx.instance(element.upcast());
            let members: Vec<ElementId> = instance
                .getters
                .iter()
                .map(|g| g.raw())
                .chain(instance.setters.iter().map(|s| s.raw()))
                .chain(instance.methods.iter().map(|m| m.raw()))
                .collect();
            for m in members {
                if member::is_static(&ctx, ElemRef::Base(m)) {
                    continue;
                }
                let Some(name) = member::lookup_name(&ctx, ElemRef::Base(m)) else {
                    continue;
                };
                // Dart `isConflictingName`.
                if !name.starts_with('_') {
                    continue;
                }
                let names = mixed_in_names.entry(library).or_default();
                if let Some(conflicting) = names.get(&name).cloned() {
                    let display = name.strip_suffix('=').unwrap_or(&name);
                    self.report_at(
                        diag::private_collision_in_mixin_application(
                            display,
                            &mixin_lexeme,
                            &conflicting,
                        ),
                        mixin_type,
                    );
                    return;
                }
                names.insert(name.clone(), mixin_lexeme.clone());
                let inherited_member = inheritance.get_member_with(
                    declared_super_element,
                    Name::new(&ctx, Some(library), &name),
                    GetMemberOptions {
                        concrete: true,
                        ..GetMemberOptions::default()
                    },
                );
                if let Some(inherited_member) = inherited_member {
                    let display = name.strip_suffix('=').unwrap_or(&name);
                    let enclosing = member::enclosing_element(&ctx, inherited_member)
                        .map(|e| self.name_of(e))
                        .unwrap_or_default();
                    self.report_at(
                        diag::private_collision_in_mixin_application(
                            display,
                            &mixin_lexeme,
                            &enclosing,
                        ),
                        mixin_type,
                    );
                    return;
                }
            }
        }
    }

    /// Dart `_checkForMultiplePrimaryConstructorBodyDeclarations`.
    fn check_for_multiple_primary_constructor_body_declarations(
        &mut self,
        members: &[Id<ClassMember>],
    ) {
        let ast = self.ast;
        let bodies: Vec<Id<PrimaryConstructorBody>> = members
            .iter()
            .filter_map(|m| ast.cast::<PrimaryConstructorBody>(m.raw()))
            .collect();
        for &body in bodies.iter().skip(1) {
            self.report_at_token(
                diag::multiple_primary_constructor_body_declarations(),
                ast[body].this_keyword,
            );
        }
    }

    /// Dart `_checkForNoDefaultSuperConstructorImplicit`.
    fn check_for_no_default_super_constructor_implicit(&mut self, element: ElementId) {
        let ctx = self.ctx;
        let Some(class) = element.cast::<InterfaceElement>() else {
            return;
        };
        // do nothing if there is explicit constructor
        let Some(&first) = ctx.interface(class).constructors.first() else {
            return;
        };
        if self.first_fragment_has(
            first.raw(),
            FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_ORIGIN_DECLARATION,
        ) {
            return;
        }
        // prepare super
        let Some(super_type) = ctx.element_supertype(class) else {
            return;
        };
        let Some(super_element) = ctx.interface_element(super_type) else {
            return;
        };
        let (offset, length) = self.diagnostic_range(element);
        let class_name = self.name_of(element);
        // try to find default generative super constructor
        if let Some(unnamed) = lookup::get_named_constructor(&ctx, super_element, "new") {
            let u = unnamed.raw();
            if self.first_fragment_has(u, FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY) {
                let super_name = self.name_of(super_element.raw());
                self.report(
                    diag::non_generative_implicit_constructor(
                        &super_name,
                        &class_name,
                        element_arg(&ctx, u),
                    )
                    .at_offset(offset, length),
                );
                return;
            }
            // Dart `isDefaultConstructor`: no required parameters.
            let is_default = ctx
                .get(unnamed)
                .formal_params
                .iter()
                .all(|&p| !ctx.get(p).kind.is_required());
            if is_default {
                return;
            }
        }
        let super_name = self.name_of(super_element.raw());
        let super_uri = self
            .library_of(super_element.raw())
            .map(|l| ctx.library_uri(l))
            .unwrap_or_default();
        // Don't report this diagnostic for non-subtypable classes because the
        // real problem was already reported.
        if !TypeProvider::is_non_subtypable_class(&super_name, super_uri) {
            self.report(
                diag::no_default_super_constructor_implicit(
                    type_arg(&ctx, super_type),
                    &class_name,
                )
                .at_offset(offset, length),
            );
        }
    }

    /// Dart `_checkForNoGenerativeConstructorsInSuperclass`.
    fn check_for_no_generative_constructors_in_superclass(
        &mut self,
        superclass: Option<Id<NamedType>>,
    ) -> bool {
        let Some(enclosing_class) = self.enclosing_class else {
            return false;
        };
        let ctx = self.ctx;
        let Some(super_type) = ctx.element_supertype(enclosing_class) else {
            return false;
        };
        let is_factory = |c: &EId<dartr_element::ConstructorElement>| -> bool {
            self.first_fragment_has(c.raw(), FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY)
        };
        // A class with no generative constructors *can* be extended if the
        // subclass has only factory constructors.
        if ctx
            .interface(enclosing_class)
            .constructors
            .iter()
            .all(is_factory)
        {
            return false;
        }
        let Some(super_element) = ctx.interface_element(super_type) else {
            return false;
        };
        let super_constructors = &ctx.interface(super_element).constructors;
        // Exclude empty constructor set, which indicates other errors occurred.
        if super_constructors.is_empty() {
            return false;
        }
        if super_constructors.iter().all(is_factory) {
            let subclass_name = self.name_of(enclosing_class.raw());
            let superclass_name = self.name_of(super_element.raw());
            if let Some(superclass) = superclass {
                self.report_at(
                    diag::no_generative_constructors_in_superclass(
                        &subclass_name,
                        &superclass_name,
                    ),
                    superclass,
                );
            }
            return true;
        }
        false
    }

    /// Dart `_checkForNonCovariantTypeParameterPositionInRepresentationType`.
    fn check_for_non_covariant_type_parameter_position_in_representation_type(
        &mut self,
        node: Id<ExtensionTypeDeclaration>,
        element: EId<ExtensionTypeElement>,
    ) {
        let ast = self.ast;
        let Some(type_parameters) = name_part_type_parameters(ast, ast[node].name_part) else {
            return;
        };
        let ctx = self.ctx;
        if ctx.get(element).fields.is_empty() {
            return;
        }
        let representation_type = ctx.extension_type_representation(element);
        for &type_parameter in ast.list(ast[type_parameters].type_parameters) {
            let Some(tp) = self
                .declared_fragment(type_parameter)
                .and_then(|f| self.element_of(f))
                .and_then(|e| e.cast::<TypeParameterElement>())
            else {
                continue;
            };
            let mut variance = Variance::Covariant;
            if self.non_covariant_type_parameter_position(representation_type, tp, &mut variance) {
                self.report_at(
                    diag::non_covariant_type_parameter_position_in_representation_type(),
                    type_parameter,
                );
            }
        }
    }

    /// Dart `NonCovariantTypeParameterPositionVisitor([typeParameter],
    /// initialVariance:)` applied to [t].
    fn non_covariant_type_parameter_position(
        &self,
        t: TypeId,
        type_parameter: EId<TypeParameterElement>,
        variance: &mut Variance,
    ) -> bool {
        let ctx = self.ctx;
        match *ctx.ty(t) {
            TypeKind::Function(f) => {
                if self.non_covariant_type_parameter_position(f.ret, type_parameter, variance) {
                    return true;
                }
                let old_variance = *variance;
                *variance = Variance::Invariant;
                for &p in ctx.list(f.type_params) {
                    if let Some(bound) = ctx.get(p).bound.get()
                        && self.non_covariant_type_parameter_position(
                            bound,
                            type_parameter,
                            variance,
                        )
                    {
                        return true;
                    }
                }
                *variance = old_variance.combine(Variance::Contravariant);
                for p in ctx.list(f.params) {
                    if self.non_covariant_type_parameter_position(p.ty, type_parameter, variance) {
                        return true;
                    }
                }
                *variance = old_variance;
                false
            }
            TypeKind::Interface { args, .. } => ctx
                .list(args)
                .iter()
                .any(|&a| self.non_covariant_type_parameter_position(a, type_parameter, variance)),
            TypeKind::Record {
                positional, named, ..
            } => {
                ctx.list(positional).iter().any(|&a| {
                    self.non_covariant_type_parameter_position(a, type_parameter, variance)
                }) || ctx.list(named).iter().any(|n| {
                    self.non_covariant_type_parameter_position(n.ty, type_parameter, variance)
                })
            }
            TypeKind::TypeParameter { param, .. } => {
                *variance != Variance::Covariant && param == type_parameter
            }
            _ => false,
        }
    }

    /// Dart `_checkForNotInitializedFieldDeclaration`.
    fn check_for_not_initialized_field_declaration(
        &mut self,
        field_declaration: Id<FieldDeclaration>,
        has_generative_constructor: bool,
    ) {
        let ast = self.ast;
        let tokens = &ast.tokens;
        let fd = &ast[field_declaration];
        let variable_list = &ast[fd.fields];
        let keyword = variable_list.keyword.map(|k| tokens.ty(k));
        let variables = ast.list(variable_list.variables);

        if keyword == Some(Keyword::CONST) {
            for &variable in variables {
                if ast[variable].initializer.is_none() {
                    let name = ast[variable].name;
                    self.report_at_token(diag::const_not_initialized(tokens.lexeme(name)), name);
                }
            }
            return;
        }

        if fd.abstract_keyword.is_some()
            || fd.external_keyword.is_some()
            || variable_list.late_keyword.is_some()
        {
            return;
        }

        let is_instance_field = fd.static_keyword.is_none();
        if is_instance_field {
            // [FfiVerifier] reports [fieldMustBeExternalInStruct].
            if self.is_enclosing_class_ffi("Struct") || self.is_enclosing_class_ffi("Union") {
                return;
            }
            // If there is a constructor, we use [ConstructorFieldsVerifier].
            if has_generative_constructor {
                return;
            }
        }

        for &variable in variables {
            let v = &ast[variable];
            if v.initializer.is_some() {
                continue;
            }
            let name = tokens.lexeme(v.name);
            if keyword == Some(Keyword::FINAL) {
                self.report_at_token(diag::final_not_initialized(name), v.name);
                continue;
            }
            let Some(element) = self
                .declared_fragment(variable)
                .and_then(|f| self.element_of(f))
            else {
                continue;
            };
            let t = crate::element_ext::variable_type(&self.ctx, element);
            if self.type_system.is_potentially_non_nullable(t) {
                if is_instance_field {
                    self.report_at_token(
                        diag::not_initialized_non_nullable_instance_field(name),
                        v.name,
                    );
                } else {
                    self.report_at_token(diag::not_initialized_non_nullable_variable(name), v.name);
                }
            }
        }
    }

    /// Dart `_checkForNotInitializedFieldDeclarations`.
    fn check_for_not_initialized_field_declarations(
        &mut self,
        element: ElementId,
        members: &[Id<ClassMember>],
    ) {
        let ctx = self.ctx;
        let ast = self.ast;
        let mut has_generative_constructor = false;
        if let Some(interface) = element.cast::<InterfaceElement>() {
            has_generative_constructor = ctx.interface(interface).constructors.iter().any(|c| {
                !self.first_fragment_has(c.raw(), FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY)
                    && self.first_fragment_has(
                        c.raw(),
                        FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_ORIGIN_DECLARATION,
                    )
            });
        }
        // Primary constructor body is an intention to have a constructor.
        has_generative_constructor |= members
            .iter()
            .any(|m| ast.is::<PrimaryConstructorBody>(m.raw()));
        for &member in members {
            if let Some(field_declaration) = ast.cast::<FieldDeclaration>(member.raw()) {
                self.check_for_not_initialized_field_declaration(
                    field_declaration,
                    has_generative_constructor,
                );
            }
        }
    }

    /// Dart `_checkForOnClauseErrorCodes`.
    fn check_for_on_clause_error_codes(&mut self, on_clause: Option<Id<MixinOnClause>>) -> bool {
        let Some(on_clause) = on_clause else {
            return false;
        };
        let ast = self.ast;
        let mut problem_reported = false;
        for &named_type in ast.list(ast[on_clause].superclass_constraints) {
            if self.named_type_interface(named_type).is_some()
                && (self.check_for_extends_or_implements_disallowed_class(named_type)
                    || self.check_for_extends_or_implements_deferred_class(
                        named_type,
                        diag::mixin_super_class_constraint_deferred_class(),
                    ))
            {
                problem_reported = true;
            }
        }
        problem_reported
    }

    /// Dart `_checkForRepeatedType`: [accumulated] is the declaration whose
    /// set ([set]) accumulates the elements.
    fn check_for_repeated_type(
        &mut self,
        accumulated: ElementId,
        set: RepeatedTypeSet,
        named_types: Option<NodeList<NamedType>>,
        code: fn(&str) -> LocatableDiagnostic,
    ) {
        let Some(named_types) = named_types else {
            return;
        };
        let ast = self.ast;
        for &named_type in ast.list(named_types) {
            let Some(element) = self.named_type_interface(named_type) else {
                continue;
            };
            let accumulated_elements = match set {
                RepeatedTypeSet::Implements => self.library_context.set_of_implements(accumulated),
                RepeatedTypeSet::On => self.library_context.set_of_on(accumulated),
            };
            let added = accumulated_elements.insert(element.raw());
            if !added {
                let name = self.name_of(element.raw());
                self.report_at(code(&name), named_type);
            }
        }
    }

    /// Dart `_checkForSealedSupertypeOutsideOfLibrary`.
    fn check_for_sealed_supertype_outside_of_library(&mut self, supertypes: &[Id<NamedType>]) {
        for &named_type in supertypes {
            if let Some(element) = self.named_type_interface(named_type) {
                let e = element.raw();
                if self.is_sealed(e) && self.library_of(e) != Some(self.current_library()) {
                    let name = self.name_of(e);
                    self.report_at(
                        diag::sealed_class_subtype_outside_of_library(&name),
                        named_type,
                    );
                }
            }
        }
    }

    /// Dart `_checkForTypeAliasCannotReferenceItself`.
    fn check_for_type_alias_cannot_reference_itself(
        &mut self,
        name_token: TokenId,
        fragment: FragmentId,
    ) {
        if fragment.tag() != Tag::TypeAlias {
            return;
        }
        let f: FId<dartr_element::TypeAliasFragment> = FId::from_raw(fragment);
        if self.ctx.fragment(f).has_self_reference.get() {
            self.report_at_token(diag::type_alias_cannot_reference_itself(), name_token);
        }
    }

    /// Dart `_checkForWrongTypeParameterVarianceInSuperinterfaces`.
    fn check_for_wrong_type_parameter_variance_in_superinterfaces(&mut self) {
        let Some(enclosing_class) = self.enclosing_class else {
            return;
        };
        let ctx = self.ctx;
        let mut super_interfaces: Vec<TypeId> = Vec::new();
        super_interfaces.extend(ctx.element_supertype(enclosing_class));
        super_interfaces.extend(ctx.element_interfaces(enclosing_class).iter().copied());
        super_interfaces.extend(ctx.element_mixins(enclosing_class).iter().copied());
        if enclosing_class.raw().tag() == Tag::Mixin {
            super_interfaces.extend(
                ctx.element_superclass_constraints(enclosing_class)
                    .iter()
                    .copied(),
            );
        }
        let type_parameters = ctx.instance(enclosing_class.upcast()).type_params.clone();
        for super_interface in super_interfaces {
            for &type_parameter in &type_parameters {
                let super_variance = self.compute_variance_in_type(type_parameter, super_interface);
                let declared = ctx.get(type_parameter).variance;
                let variance = declared.map(flow_variance).unwrap_or(Variance::Covariant);
                // If `X` is an `out` type parameter, it can only occur in `S`
                // in an covariant or unrelated position, etc.
                if super_variance.greater_than_or_equal(variance) {
                    continue;
                }
                let name = ctx.element_name(type_parameter.raw()).unwrap_or("");
                let (offset, length) = self.diagnostic_range(type_parameter.raw());
                let d = match declared {
                    Some(declared) => {
                        diag::wrong_explicit_type_parameter_variance_in_superinterface(
                            name,
                            declared.keyword(),
                            variance_keyword(super_variance),
                            type_arg(&ctx, super_interface),
                        )
                    }
                    None => diag::wrong_type_parameter_variance_in_superinterface(
                        name,
                        type_arg(&ctx, super_interface),
                    ),
                };
                self.report(d.at_offset(offset, length));
            }
        }
    }

    /// Dart `TypeParameterElementImpl.computeVarianceInType(type)`.
    fn compute_variance_in_type(
        &self,
        type_parameter: EId<TypeParameterElement>,
        t: TypeId,
    ) -> Variance {
        let ctx = self.ctx;
        match *ctx.ty(t) {
            TypeKind::TypeParameter { param, .. } => {
                if param == type_parameter {
                    Variance::Covariant
                } else {
                    Variance::Unrelated
                }
            }
            TypeKind::Interface { element, args, .. } => {
                let parameters = &ctx.instance(element.upcast()).type_params;
                let mut result = Variance::Unrelated;
                for (i, &argument) in ctx.list(args).iter().enumerate() {
                    let Some(&parameter) = parameters.get(i) else {
                        break;
                    };
                    let parameter_variance = ctx
                        .get(parameter)
                        .variance
                        .map(flow_variance)
                        .unwrap_or(Variance::Covariant);
                    result = result.meet(
                        parameter_variance
                            .combine(self.compute_variance_in_type(type_parameter, argument)),
                    );
                }
                result
            }
            TypeKind::Function(f) => {
                let mut result = self.compute_variance_in_type(type_parameter, f.ret);
                for &p in ctx.list(f.type_params) {
                    // If [p] is referenced in the bound at all, it makes the
                    // variance of [p] in the entire type invariant.
                    if let Some(bound) = ctx.get(p).bound.get()
                        && !self
                            .compute_variance_in_type(type_parameter, bound)
                            .is_unrelated()
                    {
                        result = Variance::Invariant;
                    }
                }
                for p in ctx.list(f.params) {
                    result = result.meet(
                        Variance::Contravariant
                            .combine(self.compute_variance_in_type(type_parameter, p.ty)),
                    );
                }
                result
            }
            _ => Variance::Unrelated,
        }
    }

    /// Dart `_checkImplementsSuperClass`.
    fn check_implements_super_class(&mut self, implements_clause: Option<Id<ImplementsClause>>) {
        let Some(implements_clause) = implements_clause else {
            return;
        };
        let ctx = self.ctx;
        let Some(super_element) = self
            .enclosing_class
            .and_then(|c| ctx.element_supertype(c))
            .and_then(|s| ctx.interface_element(s))
        else {
            return;
        };
        let ast = self.ast;
        for &interface in ast.list(ast[implements_clause].interfaces) {
            if self.named_type_interface(interface) == Some(super_element) {
                self.report_at(
                    diag::implements_super_class(element_arg(&ctx, super_element.raw())),
                    interface,
                );
            }
        }
    }

    /// Dart `_checkImplementsSuperClassConstraint`.
    fn check_implements_super_class_constraint(
        &mut self,
        on_clause: Option<Id<MixinOnClause>>,
        implements_clause: Option<Id<ImplementsClause>>,
    ) {
        let (Some(on_clause), Some(implements_clause)) = (on_clause, implements_clause) else {
            return;
        };
        if self.feature_enabled(ExperimentalFlag::Augmentations) {
            return;
        }
        let ast = self.ast;
        let ctx = self.ctx;
        let on_elements: IndexSet<EId<InterfaceElement>> = ast
            .list(ast[on_clause].superclass_constraints)
            .iter()
            .filter_map(|&n| self.named_type_interface(n))
            .collect();
        for &interface in ast.list(ast[implements_clause].interfaces) {
            if let Some(element) = self.named_type_interface(interface)
                && on_elements.contains(&element)
            {
                self.report_at(
                    diag::implements_super_class_constraint(element_arg(&ctx, element.raw())),
                    interface,
                );
            }
        }
    }

    /// Dart `_checkMixinInheritance`.
    fn check_mixin_inheritance(
        &mut self,
        declaration: ElementId,
        declared_fragment: FragmentId,
        node: Id<MixinDeclaration>,
    ) {
        let ast = self.ast;
        let n = &ast[node];
        let on_clause = n.on_clause;
        let implements_clause = n.implements_clause;
        // Only check for all of the inheritance logic around clauses if there
        // isn't an error code such as "Cannot implement double" already.
        if !self.check_for_on_clause_error_codes(on_clause)
            && !self.check_for_implements_clause_error_codes(implements_clause)
        {
            self.check_for_repeated_type(
                declaration,
                RepeatedTypeSet::On,
                on_clause.map(|c| ast[c].superclass_constraints),
                diag::on_repeated,
            );
            self.check_for_repeated_type(
                declaration,
                RepeatedTypeSet::Implements,
                implements_clause.map(|c| ast[c].interfaces),
                diag::implements_repeated,
            );
            self.check_implements_super_class_constraint(on_clause, implements_clause);
            self.check_for_conflicting_generics(declared_fragment, n.name);
            self.check_for_base_class_or_mixin_implemented_outside_of_library(implements_clause);
            self.check_for_final_supertype_outside_of_library(None, implements_clause, on_clause);
            let mut supertypes: Vec<Id<NamedType>> = Vec::new();
            if let Some(i) = implements_clause {
                supertypes.extend(ast.list(ast[i].interfaces).iter().copied());
            }
            if let Some(o) = on_clause {
                supertypes.extend(ast.list(ast[o].superclass_constraints).iter().copied());
            }
            self.check_for_sealed_supertype_outside_of_library(&supertypes);
        }
    }

    /// Dart `_checkMixinsSuperClass`.
    fn check_mixins_super_class(&mut self, with_clause: Option<Id<WithClause>>) {
        let Some(with_clause) = with_clause else {
            return;
        };
        let ctx = self.ctx;
        let Some(super_element) = self
            .enclosing_class
            .and_then(|c| ctx.element_supertype(c))
            .and_then(|s| ctx.interface_element(s))
        else {
            return;
        };
        let ast = self.ast;
        for &mixin in ast.list(ast[with_clause].mixin_types) {
            if self.named_type_interface(mixin) == Some(super_element) {
                self.report_at(
                    diag::mixins_super_class(element_arg(&ctx, super_element.raw())),
                    mixin,
                );
            }
        }
    }

    /// Dart `_isWasm`.
    fn is_wasm(&self, imported_library: Option<EId<LibraryElement>>) -> bool {
        let ctx = self.ctx;
        let imported_uri = imported_library.map(|l| ctx.library_uri(l));
        if imported_uri != Some("dart:_wasm") && imported_uri != Some("dart:_js_interop_wasm") {
            return false;
        }
        let importing_uri = ctx.library_uri(self.current_library());
        importing_uri == "package:js/js.dart" || importing_uri.starts_with("package:ui/")
    }

    /// Dart `_mayIgnoreClassModifiers`.
    fn may_ignore_class_modifiers(&self, super_library: Option<EId<LibraryElement>>) -> bool {
        let ctx = self.ctx;
        let Some(super_library) = super_library else {
            return false;
        };
        let uri = ctx.library_uri(super_library);
        // Only modifiers in platform libraries can be ignored.
        if !uri.starts_with("dart:") {
            return false;
        }
        // Modifiers in 'dart:ffi' can't be ignored in pre-feature code.
        if uri == "dart:ffi" {
            return false;
        }
        // Other platform libraries can ignore modifiers.
        if self.is_in_system_library {
            return true;
        }
        // Libraries predating class modifiers can ignore platform modifiers.
        !self.feature_enabled(ExperimentalFlag::ClassModifiers)
    }

    /// Dart `_reportForMultipleCombinators`.
    fn report_for_multiple_combinators(&mut self, combinators: NodeList<Combinator>) {
        let ast = self.ast;
        let list = ast.list(combinators);
        if list.len() > 1 {
            let offset = ast.offset(list[0]);
            let end = ast.end(list[list.len() - 1]);
            self.report(
                diag::multiple_combinators()
                    .at_offset(offset as usize, end.saturating_sub(offset) as usize),
            );
        }
    }

    /// Dart `_isEnclosingClassFfiStruct` / `_isEnclosingClassFfiUnion`.
    fn is_enclosing_class_ffi(&self, name: &str) -> bool {
        let ctx = self.ctx;
        let Some(super_class) = self
            .enclosing_class
            .and_then(|c| ctx.element_supertype(c))
            .and_then(|s| ctx.interface_element(s))
        else {
            return false;
        };
        self.library_of(super_class.raw())
            .is_some_and(|l| ctx.library_uri(l) == "dart:ffi")
            && ctx.element_name(super_class.raw()) == Some(name)
    }

    // ------------------------------------------------------------ model access

    /// Dart `node.declaredFragment`.
    fn declared_fragment(&self, node: impl Into<NodeId>) -> Option<FragmentId> {
        self.tables.declared_fragment.get(node).copied()
    }

    /// Dart `fragment.element`.
    fn element_of(&self, fragment: FragmentId) -> Option<ElementId> {
        self.ctx
            .fragment_data(fragment)
            .and_then(|f| f.element.try_get().copied())
    }

    /// Dart `element.firstFragment`.
    fn first_fragment(&self, element: ElementId) -> FragmentId {
        self.ctx
            .element_data(element)
            .map(|d| d.first_fragment)
            .expect("element data")
    }

    /// The instance data of an instance fragment (class, enum, mixin,
    /// extension, extension type).
    fn instance_fragment(
        &self,
        fragment: FragmentId,
    ) -> Option<&'_ dartr_element::InstanceFragmentData> {
        let ctx = self.ctx;
        Some(match fragment.tag() {
            Tag::Class => {
                &ctx.fragment(FId::<dartr_element::ClassFragment>::from_raw(fragment))
                    .interface
                    .instance
            }
            Tag::Enum => {
                &ctx.fragment(FId::<dartr_element::EnumFragment>::from_raw(fragment))
                    .interface
                    .instance
            }
            Tag::Mixin => {
                &ctx.fragment(FId::<dartr_element::MixinFragment>::from_raw(fragment))
                    .interface
                    .instance
            }
            Tag::ExtensionType => {
                &ctx.fragment(FId::<dartr_element::ExtensionTypeFragment>::from_raw(
                    fragment,
                ))
                .interface
                .instance
            }
            Tag::Extension => {
                &ctx.fragment(FId::<dartr_element::ExtensionFragment>::from_raw(fragment))
                    .instance
            }
            _ => return None,
        })
    }

    /// Dart `fragment.typeParameters`.
    fn fragment_type_parameters(&self, fragment: FragmentId) -> Vec<FragmentId> {
        self.instance_fragment(fragment)
            .map(|i| i.type_params.iter().map(|p| p.raw()).collect())
            .unwrap_or_default()
    }

    /// The getters of an instance element (Dart `element.getters`).
    fn instance_getters(&self, element: ElementId) -> Vec<ElementId> {
        match element.tag() {
            Tag::Class | Tag::Enum | Tag::Mixin | Tag::ExtensionType | Tag::Extension => self
                .ctx
                .instance(EId::from_raw(element))
                .getters
                .iter()
                .map(|g| g.raw())
                .collect(),
            _ => Vec::new(),
        }
    }

    pub(super) fn fragment_has(&self, fragment: FragmentId, flag: FragmentFlags) -> bool {
        self.ctx
            .fragment_data(fragment)
            .is_some_and(|f| f.flags.has(flag))
    }

    fn first_fragment_has(&self, element: ElementId, flag: FragmentFlags) -> bool {
        crate::element_ext::first_fragment_flags(&self.ctx, element).contains(flag)
    }

    fn element_has(&self, element: ElementId, flag: ElementFlags) -> bool {
        self.ctx
            .element_data(element)
            .is_some_and(|d| d.flags.has(flag))
    }

    /// Dart `ClassElementImpl.isBase`, `MixinElementImpl.isBase`.
    fn is_base(&self, e: ElementId) -> bool {
        match e.tag() {
            Tag::Class => self.element_has(e, ElementFlags::CLASS_ELEMENT_IS_BASE),
            Tag::Mixin => self.first_fragment_has(e, FragmentFlags::MIXIN_FRAGMENT_IS_BASE),
            _ => false,
        }
    }

    /// Dart `ClassElementImpl.isSealed` (`false` for other elements).
    fn is_sealed(&self, e: ElementId) -> bool {
        e.tag() == Tag::Class && self.first_fragment_has(e, FragmentFlags::CLASS_FRAGMENT_IS_SEALED)
    }

    /// Dart `ClassElementImpl.isMixinClass` (`false` for other elements).
    fn is_mixin_class(&self, e: ElementId) -> bool {
        e.tag() == Tag::Class
            && self.first_fragment_has(e, FragmentFlags::CLASS_FRAGMENT_IS_MIXIN_CLASS)
    }

    /// Dart `ClassElementImpl.isMixinApplication`.
    fn is_mixin_application(&self, e: ElementId) -> bool {
        e.tag() == Tag::Class
            && self.first_fragment_has(e, FragmentFlags::CLASS_FRAGMENT_IS_MIXIN_APPLICATION)
    }

    /// Dart `element.library`.
    fn library_of(&self, e: ElementId) -> Option<EId<LibraryElement>> {
        self.ctx.element_data(e).and_then(|d| d.library)
    }

    /// Dart `element.name!` (empty when absent).
    fn name_of(&self, e: ElementId) -> String {
        self.ctx.element_name(e).unwrap_or("").to_string()
    }

    /// Dart `_featureSet.isEnabled(feature)` (the current library).
    fn feature_enabled(&self, flag: ExperimentalFlag) -> bool {
        crate::scope::library_feature_enabled(&self.ctx, self.current_library(), flag)
    }

    /// Dart `Element.isWildcardVariable` of a type parameter.
    fn is_wildcard_variable(&self, type_parameter: EId<TypeParameterElement>) -> bool {
        self.ctx.element_name(type_parameter.raw()) == Some("_")
            && self.library_of(type_parameter.raw()).is_some_and(|l| {
                crate::scope::library_feature_enabled(
                    &self.ctx,
                    l,
                    ExperimentalFlag::WildcardVariables,
                )
            })
    }

    /// Dart `element.diagnosticRange(source)`: the name of the first
    /// fragment of the non-synthetic element.
    fn diagnostic_range(&self, element: ElementId) -> (usize, usize) {
        let ctx = self.ctx;
        let mut e = element;
        // Dart `nonSynthetic` of accessors induced by a variable.
        if matches!(e.tag(), Tag::Getter | Tag::Setter)
            && self.first_fragment_has(
                e,
                FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE,
            )
        {
            let variable = match ctx.any(e) {
                AnyElement::Getter(g) => g.variable.get(),
                AnyElement::Setter(s) => s.variable.get(),
                _ => None,
            };
            if let Some(v) = variable {
                e = v.raw();
            }
        }
        let Some(fragment) = ctx
            .element_data(e)
            .and_then(|d| ctx.fragment_data(d.first_fragment))
        else {
            return (0, 0);
        };
        match fragment.name_offset {
            Some(offset) => (
                offset as usize,
                fragment
                    .name
                    .map(|n| ctx.name_str(n).encode_utf16().count())
                    .unwrap_or(0),
            ),
            None => (0, 0),
        }
    }

    /// Dart `namedType.typeOrThrow` (`InvalidType` when not resolved).
    fn named_type_type(&self, named_type: Id<NamedType>) -> TypeId {
        self.named_type_type_opt(named_type)
            .unwrap_or(TypeId::INVALID)
    }

    /// Dart `namedType.type`.
    fn named_type_type_opt(&self, named_type: Id<NamedType>) -> Option<TypeId> {
        self.tables.annotation_type.get(named_type).copied()
    }

    /// The element of `namedType.type` when it is an interface type.
    fn named_type_interface(&self, named_type: Id<NamedType>) -> Option<EId<InterfaceElement>> {
        self.named_type_type_opt(named_type)
            .and_then(|t| self.ctx.interface_element(t))
    }

    /// Dart `NamedType.isSynthetic`.
    fn named_type_is_synthetic(&self, named_type: Id<NamedType>) -> bool {
        let n = &self.ast[named_type];
        self.ast.tokens.get(n.name).is_synthetic() && n.type_arguments.is_none()
    }

    /// Dart `NamedType.isDeferred`.
    pub(super) fn named_type_is_deferred(&self, named_type: Id<NamedType>) -> bool {
        let ctx = self.ctx;
        let Some(prefix) = self.ast[named_type].import_prefix else {
            return false;
        };
        let Some(ElemRef::Base(element)) = self.element(prefix) else {
            return false;
        };
        if element.tag() != Tag::Prefix {
            return false;
        }
        let mut fragment = ctx.element_data(element).map(|d| d.first_fragment);
        while let Some(f) = fragment {
            if f.tag() != Tag::Prefix {
                break;
            }
            let pf: FId<dartr_element::PrefixFragment> = FId::from_raw(f);
            let data = ctx.fragment(pf);
            if data.is_deferred {
                return true;
            }
            fragment = data.next_fragment;
        }
        false
    }

    /// Dart `node.representationFormalParameter?.type`.
    fn representation_formal_parameter_type(
        &self,
        node: Id<ExtensionTypeDeclaration>,
    ) -> Option<Id<TypeAnnotation>> {
        let ast = self.ast;
        let primary = ast.cast::<PrimaryConstructorDeclaration>(ast[node].name_part.raw())?;
        let first = *ast
            .list(ast[ast[primary].formal_parameters].parameters)
            .first()?;
        let regular = ast.cast::<RegularFormalParameter>(first.raw())?;
        ast[regular].type_
    }

    /// The index of the directive [node] among the directives of its kind
    /// in the unit (Dart `LibraryAnalyzer._resolveDirectives` matches them
    /// with `libraryImports` / `libraryExports` by position).
    fn directive_index<T: NodeType>(&self, node: NodeId) -> Option<usize> {
        let ast = self.ast;
        let unit = ast
            .parent(node)
            .and_then(|p| ast.cast::<CompilationUnit>(p))?;
        ast.list(ast[unit].directives)
            .iter()
            .filter(|d| ast.is::<T>(d.raw()))
            .position(|d| d.raw() == node)
    }

    /// Dart `NamespaceBuilder().createExportNamespaceForDirective2(...)`:
    /// the export namespace of [library] with [combinators] applied.
    fn export_namespace_for_directive(
        &self,
        library: EId<LibraryElement>,
        combinators: &[NamespaceCombinator],
    ) -> IndexMap<String, ElementId> {
        let ctx = self.ctx;
        let mut result = IndexMap::new();
        let Some(namespace) = ctx.get(library).export_namespace.try_get() else {
            return result;
        };
        for (&name, &element) in &namespace.defined_names {
            let name = ctx.name_str(name);
            if combinators_allow(&ctx, combinators, name) {
                result.insert(name.to_string(), element);
            }
        }
        result
    }

    /// Dart `StringLiteral.stringValue` of a simple string literal.
    fn string_literal_value(&self, node: NodeId) -> String {
        let ast = self.ast;
        match ast.cast::<SimpleStringLiteral>(node) {
            Some(s) => ast[s].value.to_string(),
            None => String::new(),
        }
    }

    // ------------------------------------------------------------ helpers that other sections call

    /// Dart `_checkAugmentationWithoutDeclaration(errorToken, fragment)`.
    pub(crate) fn check_augmentation_without_declaration(
        &mut self,
        error_token: Option<TokenId>,
        fragment: FragmentId,
    ) {
        let Some(error_token) = error_token else {
            return;
        };
        let ctx = self.ctx;
        let Some(data) = ctx.fragment_data(fragment) else {
            return;
        };
        if data.previous_fragment.is_some() {
            return;
        }
        let Some(element) = self.element_of(fragment) else {
            return;
        };
        let previous = ctx
            .element_data(element)
            .and_then(|d| d.previous_fragment_of_different_kind);
        match previous {
            Some(previous)
                if previous.tag() == Tag::Class
                    && self.fragment_has(
                        previous,
                        FragmentFlags::CLASS_FRAGMENT_IS_MIXIN_APPLICATION,
                    ) =>
            {
                self.report_at_token(diag::augmentation_of_mixin_application_class(), error_token);
            }
            Some(previous) => {
                let previous_kind = self
                    .element_of(previous)
                    .map(|e| e.kind().display_name())
                    .unwrap_or("");
                self.report_at_token(
                    diag::augmentation_of_different_declaration_kind(
                        previous_kind,
                        element.kind().display_name(),
                    ),
                    error_token,
                );
            }
            None => {
                self.report_at_token(diag::augmentation_without_declaration(), error_token);
            }
        }
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
        if in_augmentation != in_introductory {
            if in_augmentation {
                if let Some(modifier_token) = modifier_token {
                    self.report_at_token(
                        diag::augmentation_modifier_extra(modifier_name),
                        modifier_token,
                    );
                }
            } else {
                self.report_at_token(
                    diag::augmentation_modifier_missing(modifier_name),
                    augment_keyword,
                );
            }
        }
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
        if !self.fragment_has(fragment, FragmentFlags::FRAGMENT_IS_AUGMENTATION) {
            return;
        }
        let ctx = self.ctx;
        let ast = self.ast;
        let introductory_count = first_type_parameters
            .iter()
            .take_while(|&&p| {
                !self.fragment_has(
                    p,
                    FragmentFlags::TYPE_PARAMETER_FRAGMENT_IS_ORIGIN_OTHER_FRAGMENT_OF_ENCLOSING,
                )
            })
            .count();

        // If no type parameter nodes, but introductory has type parameters.
        let Some(type_parameter_list) = type_parameter_list else {
            if introductory_count != 0 {
                self.report_at_token(
                    diag::augmentation_type_parameter_count(),
                    name_or_keyword_token,
                );
            }
            return;
        };
        let type_parameters = ast.list(ast[type_parameter_list].type_parameters);

        // If the number of type parameters does not match, it is an error.
        if type_parameters.len() > introductory_count {
            let name = ast[type_parameters[introductory_count]].name;
            self.report_at_token(diag::augmentation_type_parameter_count(), name);
            return;
        } else if type_parameters.len() < introductory_count {
            let right_bracket = ast[type_parameter_list].right_bracket;
            self.report_at_token(diag::augmentation_type_parameter_count(), right_bracket);
            return;
        }

        for (&first_type_parameter, &type_parameter_node) in
            first_type_parameters.iter().zip(type_parameters)
        {
            let node_name = ast[type_parameter_node].name;
            let first_name = ctx
                .fragment_data(first_type_parameter)
                .and_then(|f| f.name)
                .map(|n| ctx.name_str(n));
            if Some(ast.tokens.lexeme(node_name)) != first_name {
                self.report_at_token(diag::augmentation_type_parameter_name(), node_name);
            }
            if let Some(bound_node) = ast[type_parameter_node].bound {
                let first_bound = self
                    .element_of(first_type_parameter)
                    .and_then(|e| e.cast::<TypeParameterElement>())
                    .and_then(|e| ctx.get(e).bound.get());
                let bound_type = self.tables.annotation_type.get(bound_node).copied();
                let same = match (first_bound, bound_type) {
                    (Some(a), Some(b)) => self.type_system.dart_eq(b, a),
                    _ => false,
                };
                if !same {
                    self.report_at(diag::augmentation_type_parameter_bound(), bound_node);
                }
            }
        }
    }

    /// Dart `_checkForBuiltInIdentifierAsName(token, code)`; [code] builds
    /// the diagnostic from the name.
    pub(crate) fn check_for_built_in_identifier_as_name(
        &mut self,
        token: TokenId,
        code: fn(&str) -> LocatableDiagnostic,
    ) {
        let ty = self.ast.tokens.ty(token);
        let lexeme = self.ast.tokens.lexeme(token);
        if ty.is_keyword() && !ty.is_pseudo() {
            self.report_at_token(code(lexeme), token);
            return;
        }
        if self.feature_enabled(ExperimentalFlag::Variance)
            && matches!(ty, Keyword::INOUT | Keyword::OUT)
        {
            self.report_at_token(code(lexeme), token);
        }
    }

    /// Dart `_checkForMainFunction1(nameToken, declaredFragment)`.
    pub(crate) fn check_for_main_function1(
        &mut self,
        name_token: TokenId,
        declared_fragment: FragmentId,
    ) {
        let ctx = self.ctx;
        let Some(data) = ctx.fragment_data(declared_fragment) else {
            return;
        };
        // We should only check exported declarations, i.e. top-level.
        if data
            .enclosing_fragment
            .is_none_or(|f| f.tag() != Tag::Library)
        {
            return;
        }
        if data.name.map(|n| ctx.name_str(n)) != Some("main") {
            return;
        }
        if declared_fragment.tag() != Tag::TopLevelFunction {
            self.report_at_token(diag::main_is_not_function(), name_token);
        }
    }
}

/// Dart `ClassNamePart.typeName`.
fn name_part_type_name(ast: &Ast, name_part: Id<ClassNamePart>) -> TokenId {
    if let Some(p) = ast.cast::<PrimaryConstructorDeclaration>(name_part.raw()) {
        return ast[p].type_name;
    }
    let n = ast
        .cast::<NameWithTypeParameters>(name_part.raw())
        .expect("a class name part");
    ast[n].type_name
}

/// Dart `ClassNamePart.typeParameters`.
fn name_part_type_parameters(
    ast: &Ast,
    name_part: Id<ClassNamePart>,
) -> Option<Id<TypeParameterList>> {
    if let Some(p) = ast.cast::<PrimaryConstructorDeclaration>(name_part.raw()) {
        return ast[p].type_parameters;
    }
    ast.cast::<NameWithTypeParameters>(name_part.raw())
        .and_then(|n| ast[n].type_parameters)
}

/// Dart `ClassBody.members`.
fn class_body_members(ast: &Ast, body: NodeId) -> Vec<Id<ClassMember>> {
    match ast.cast::<BlockClassBody>(body) {
        Some(b) => ast.list(ast[b].members).to_vec(),
        None => Vec::new(),
    }
}

/// Dart `EnumBody.members`.
fn enum_body_members(ast: &Ast, body: NodeId) -> Vec<Id<ClassMember>> {
    match ast.cast::<BlockEnumBody>(body) {
        Some(b) => ast.list(ast[b].members).to_vec(),
        None => Vec::new(),
    }
}

/// Whether the export or import [combinators] let [name] through.
fn combinators_allow(
    ctx: &dartr_element::Ctx<'_>,
    combinators: &[NamespaceCombinator],
    name: &str,
) -> bool {
    let name = name.strip_suffix('=').unwrap_or(name);
    let matches = |names: &[dartr_element::Name]| names.iter().any(|&n| ctx.name_str(n) == name);
    for c in combinators {
        match c {
            NamespaceCombinator::Show { shown_names, .. } => {
                if !matches(shown_names) {
                    return false;
                }
            }
            NamespaceCombinator::Hide { hidden_names, .. } => {
                if matches(hidden_names) {
                    return false;
                }
            }
        }
    }
    true
}

/// The shared [Variance] of an element variance.
fn flow_variance(v: dartr_element::Variance) -> Variance {
    match v {
        dartr_element::Variance::Unrelated => Variance::Unrelated,
        dartr_element::Variance::Covariant => Variance::Covariant,
        dartr_element::Variance::Contravariant => Variance::Contravariant,
        dartr_element::Variance::Invariant => Variance::Invariant,
    }
}

/// Dart `Variance.keyword`.
fn variance_keyword(v: Variance) -> &'static str {
    match v {
        Variance::Unrelated => "",
        Variance::Covariant => "out",
        Variance::Contravariant => "in",
        Variance::Invariant => "inout",
    }
}

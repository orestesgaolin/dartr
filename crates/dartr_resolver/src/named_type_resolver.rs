// Dart source: pkg/analyzer/lib/src/dart/resolver/named_type_resolver.dart
// (NamedTypeResolver, _ErrorHelper)
// Dart source: pkg/analyzer/lib/src/dart/type_instantiation_target.dart
// (wrongNumberOfTypeArgumentsError of the type defining elements)

//! `NamedTypeResolver`: resolves a `NamedType` in the current lexical scope
//! (element, type arguments, nullability, the type of the annotation).
//!
//! Results: `ResolutionTables.element` of the `NamedType` (Dart
//! `NamedType.element`) and of its `ImportPrefixReference`
//! (`ImportPrefixReference.element`), and `ResolutionTables.annotation_type`
//! of the `NamedType` (Dart `NamedType.type`).
//!
//! Not ported: `reportDeprecatedExportUseGetter` (the scopes do not know
//! deprecated exports, see [`crate::scope`]); `dataForTesting`.

use std::sync::Arc;

use dartr_ast::{
    AsExpression, Ast, CatchClause, ClassTypeAlias, ConstructorDeclaration, ConstructorName,
    ExtendsClause, Id, ImplementsClause, ImportPrefixReference, InstanceCreationExpression,
    IsExpression, MixinOnClause, NodeId, SimpleIdentifier, TypeArgumentList, WithClause,
};
use dartr_diagnostics::{Diagnostic, DiagnosticMessage, LocatedDiagnostic, diag};
use dartr_element::{
    Ctx, EId, ElemRef, ElementId, FId, FragmentId, InterfaceElement, LibraryElement,
    LibraryFragment, Nullability, PrefixElement, Tag, TypeAliasElement, TypeId, TypeKind,
    TypeParameterElement,
};
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_typesystem::generic_inferrer::InferenceFlags;
use dartr_typesystem::type_system_operations::TypeSystemOperations;
use dartr_typesystem::{TypeExt, TypeSystem};

use crate::ast_ext::{instance_creation_is_const, token_end, token_is_synthetic};
use crate::scope::{NameScope, library_feature_enabled};
use crate::scope_context::ScopeContext;

/// What [`NamedTypeResolver::resolve`] reads and writes besides the node.
pub struct NamedTypeEnv<'r, 'a> {
    pub ctx: Ctx<'a>,
    pub type_system: TypeSystem<'a>,
    /// Dart `_scopeContext`.
    pub scope: &'r ScopeContext<'a>,
    pub tables: &'r mut dartr_element::ResolutionTables,
    /// Dart `diagnosticReporter`.
    pub diagnostics: &'r mut Vec<Diagnostic>,
}

impl NamedTypeEnv<'_, '_> {
    fn report(&mut self, d: LocatedDiagnostic) {
        self.diagnostics.push(d.into_diagnostic());
    }

    fn type_of(&self, node: impl Into<NodeId>) -> TypeId {
        self.tables
            .annotation_type
            .get(node)
            .copied()
            .unwrap_or(TypeId::INVALID)
    }
}

/// Dart `NamedTypeResolver`.
#[derive(Debug)]
pub struct NamedTypeResolver {
    library: EId<LibraryElement>,
    /// Dart `_libraryFragment`.
    library_fragment: FId<LibraryFragment>,
    /// Dart `strictInference`.
    pub strict_inference: bool,
    /// Dart `strictCasts`.
    pub strict_casts: bool,
    /// Dart `enclosingClass`: the element of the class-like declaration
    /// being resolved.
    pub enclosing_class: Option<EId<InterfaceElement>>,
    /// Dart `classHierarchy_namedType`: a direct child of an extends, with,
    /// or implements clause.
    pub class_hierarchy_named_type: Option<Id<dartr_ast::NamedType>>,
    /// Dart `withClause_namedType`.
    pub with_clause_named_type: Option<Id<dartr_ast::NamedType>>,
    /// Dart `rewriteResult`: the rewritten `ConstructorName`.
    pub rewrite_result: Option<Id<ConstructorName>>,
    /// Dart `hasErrorReported`.
    pub has_error_reported: bool,
    /// The path of the unit (Dart `diagnosticReporter.source.fullName`).
    source_path: Arc<str>,
}

impl NamedTypeResolver {
    /// Dart `NamedTypeResolver(libraryElement, libraryFragment, ...)`.
    pub fn new(
        ctx: &Ctx<'_>,
        library: EId<LibraryElement>,
        library_fragment: FId<LibraryFragment>,
        strict_inference: bool,
        strict_casts: bool,
    ) -> NamedTypeResolver {
        NamedTypeResolver {
            library,
            library_fragment,
            strict_inference,
            strict_casts,
            enclosing_class: None,
            class_hierarchy_named_type: None,
            with_clause_named_type: None,
            rewrite_result: None,
            has_error_reported: false,
            source_path: ctx.fragment(library_fragment).source.path.clone(),
        }
    }

    /// Dart `resolve(node)`: resolves the given [node] (its children must
    /// be resolved already).
    pub fn resolve(&mut self, env: &mut NamedTypeEnv<'_, '_>, ast: &mut Ast, node: Id<dartr_ast::NamedType>) {
        self.rewrite_result = None;
        self.has_error_reported = false;

        let (import_prefix, name_token) = (ast[node].import_prefix, ast[node].name);
        if let Some(import_prefix) = import_prefix {
            let prefix_token = ast[import_prefix].name;
            let prefix_name = ast.tokens.lexeme(prefix_token).to_string();
            let mut prefix_element = env.scope.lookup(&prefix_name).getter;

            // Might be shadowed by an instance member.
            // Look again to report `prefixShadowedByLocalDeclaration`.
            if prefix_element.is_none() {
                if let Some(c) = self.enclosing_class {
                    let ctx = &env.ctx;
                    let c = c.upcast();
                    prefix_element = dartr_typesystem::lookup::get_method(ctx, c, &prefix_name)
                        .map(|e| e.raw())
                        .or_else(|| {
                            dartr_typesystem::lookup::get_getter(ctx, c, &prefix_name)
                                .map(|e| e.raw())
                        })
                        .or_else(|| {
                            dartr_typesystem::lookup::get_setter(ctx, c, &prefix_name)
                                .map(|e| e.raw())
                        });
                }
            }

            match prefix_element {
                Some(e) => {
                    env.tables.element.insert(import_prefix, ElemRef::Base(e));
                }
                None => {
                    env.tables.element.remove(import_prefix);
                }
            }

            let Some(prefix_element) = prefix_element else {
                self.resolve_to_element(env, ast, node, None);
                return;
            };

            if prefix_element.is::<InterfaceElement>() || prefix_element.tag() == Tag::TypeAlias {
                self.rewrite_to_constructor_name(env, ast, node, import_prefix, prefix_element, name_token);
                return;
            }

            if let Some(prefix) = prefix_element.cast::<PrefixElement>() {
                let name = ast.tokens.lexeme(name_token).to_string();
                let element = env
                    .scope
                    .library_scopes()
                    .prefix_lookup(&env.ctx, prefix, &name)
                    .getter;
                self.resolve_to_element(env, ast, node, element);
                return;
            }

            let offset = ast.tokens.offset(prefix_token) as usize;
            let length = (token_end(ast, prefix_token) - ast.tokens.offset(prefix_token)) as usize;
            env.report(diag::prefix_shadowed_by_local_declaration(&prefix_name).at_offset(offset, length));
            env.tables.annotation_type.insert(node, TypeId::INVALID);
        } else {
            if ast.tokens.lexeme(name_token) == "void" {
                env.tables.annotation_type.insert(node, TypeId::VOID);
                return;
            }
            let name = ast.tokens.lexeme(name_token).to_string();
            let element = env.scope.lookup(&name).getter;
            self.resolve_to_element(env, ast, node, element);
        }
    }

    /// Dart `_buildTypeArguments`: exactly [parameter_count] type arguments.
    fn build_type_arguments(
        &self,
        env: &mut NamedTypeEnv<'_, '_>,
        ast: &Ast,
        node: Id<dartr_ast::NamedType>,
        argument_list: Id<TypeArgumentList>,
        parameter_count: usize,
        target_name: &str,
    ) -> Vec<TypeId> {
        let arguments = ast.list(ast[argument_list].arguments);
        let argument_count = arguments.len();
        if argument_count != parameter_count {
            // Dart `TypeInstantiationTargetTypeDefiningElement
            // .wrongNumberOfTypeArgumentsError`.
            env.report(
                diag::wrong_number_of_type_arguments(
                    target_name,
                    parameter_count as i64,
                    argument_count as i64,
                )
                .at_offset(ast.offset(node) as usize, ast.length(node) as usize),
            );
            return vec![TypeId::INVALID; parameter_count];
        }
        arguments.iter().map(|&a| env.type_of(a)).collect()
    }

    /// Dart `_getNullability`.
    fn nullability(ast: &Ast, node: Id<dartr_ast::NamedType>) -> Nullability {
        if ast[node].question.is_some() {
            Nullability::Question
        } else {
            Nullability::None
        }
    }

    /// Dart `_inferRedirectedConstructor`.
    fn infer_redirected_constructor(
        &self,
        env: &mut NamedTypeEnv<'_, '_>,
        element: EId<InterfaceElement>,
        node: NodeId,
    ) -> TypeId {
        let ctx = env.ctx;
        if Some(element) == self.enclosing_class {
            return ctx.interface_this_type(element);
        }
        let type_parameters: Vec<EId<TypeParameterElement>> =
            ctx.interface_type_parameters(element).to_vec();
        if type_parameters.is_empty() {
            return ctx.interface_this_type(element);
        }
        let Some(enclosing_class) = self.enclosing_class else {
            return ctx.interface_this_type(element);
        };
        let enclosing_library = ctx
            .element_data(enclosing_class.raw())
            .and_then(|d| d.library)
            .unwrap_or(self.library);
        let flags = InferenceFlags {
            generic_metadata_is_enabled: library_feature_enabled(
                &ctx,
                enclosing_library,
                ExperimentalFlag::GenericMetadata,
            ),
            inference_using_bounds_is_enabled: library_feature_enabled(
                &ctx,
                enclosing_library,
                ExperimentalFlag::InferenceUsingBounds,
            ),
            strict_inference: self.strict_inference,
        };
        let operations = TypeSystemOperations::new(env.type_system, self.strict_casts);
        let mut inferrer = env.type_system.setup_generic_type_inference(
            &type_parameters,
            ctx.interface_this_type(element),
            ctx.interface_this_type(enclosing_class),
            None,
            None,
            flags,
            false,
            operations,
            None,
            Some(node),
        );
        let type_arguments = inferrer.choose_final_types();
        ctx.instantiate_interface(element, &type_arguments, Nullability::None)
    }

    /// Dart `_instantiateElement`.
    fn instantiate_element(
        &mut self,
        env: &mut NamedTypeEnv<'_, '_>,
        ast: &Ast,
        node: Id<dartr_ast::NamedType>,
        element: ElementId,
    ) -> TypeId {
        let ctx = env.ctx;
        let nullability = Self::nullability(ast, node);
        let element_name = ctx
            .element_data(element)
            .and_then(|d| d.name)
            .map(|n| ctx.name_str(n))
            .unwrap_or(match element.tag() {
                Tag::Dynamic => "dynamic",
                Tag::Never => "Never",
                _ => "",
            });

        if let Some(argument_list) = ast[node].type_arguments {
            if let Some(e) = element.cast::<InterfaceElement>() {
                let count = ctx.interface_type_parameters(e).len();
                let args = self.build_type_arguments(env, ast, node, argument_list, count, element_name);
                return ctx.instantiate_interface(e, &args, nullability);
            } else if let Some(e) = element.cast::<TypeAliasElement>() {
                let count = ctx.get(e).type_params.len();
                let args = self.build_type_arguments(env, ast, node, argument_list, count, element_name);
                let t = ctx.instantiate_type_alias(e, &args, nullability);
                return self.verify_type_alias_for_context(env, ast, node, e, t);
            } else if is_instance_creation(ast, node) {
                ErrorHelper::report_new_with_non_type(env, ast, node);
                return TypeId::INVALID;
            } else if element.tag() == Tag::Dynamic {
                self.build_type_arguments(env, ast, node, argument_list, 0, element_name);
                return TypeId::DYNAMIC;
            } else if element.tag() == Tag::Never {
                self.build_type_arguments(env, ast, node, argument_list, 0, element_name);
                return ctx.never_type(nullability);
            } else if element.tag() == Tag::TypeParameter {
                self.build_type_arguments(env, ast, node, argument_list, 0, element_name);
                return TypeId::INVALID;
            } else {
                ErrorHelper::report_null_or_non_type_element(
                    env,
                    &self.source_path,
                    ast,
                    node,
                    Some(element),
                );
                return TypeId::INVALID;
            }
        }

        if let Some(e) = element.cast::<InterfaceElement>() {
            if self.with_clause_named_type == Some(node) {
                if let Some(enclosing) = self.enclosing_class {
                    for &mixin in ctx.element_mixins(enclosing) {
                        if ctx.interface_element(mixin) == Some(e) {
                            return mixin;
                        }
                    }
                }
            }
            if ErrorHelper::is_redirecting_constructor(ast, node) {
                return self.infer_redirected_constructor(env, e, node.raw());
            }
            env.type_system.instantiate_interface_to_bounds(e, nullability)
        } else if let Some(e) = element.cast::<TypeAliasElement>() {
            let t = env.type_system.instantiate_type_alias_to_bounds(e, nullability);
            self.verify_type_alias_for_context(env, ast, node, e, t)
        } else if is_instance_creation(ast, node) {
            ErrorHelper::report_new_with_non_type(env, ast, node);
            TypeId::INVALID
        } else if element.tag() == Tag::Dynamic {
            TypeId::DYNAMIC
        } else if element.tag() == Tag::Never {
            ctx.never_type(nullability)
        } else if let Some(e) = element.cast::<TypeParameterElement>() {
            env.scope.instantiate_type_parameter(e, nullability)
        } else {
            ErrorHelper::report_null_or_non_type_element(
                env,
                &self.source_path,
                ast,
                node,
                Some(element),
            );
            TypeId::INVALID
        }
    }

    /// Dart `_resolveToElement`.
    fn resolve_to_element(
        &mut self,
        env: &mut NamedTypeEnv<'_, '_>,
        ast: &mut Ast,
        node: Id<dartr_ast::NamedType>,
        element: Option<ElementId>,
    ) {
        match element {
            Some(e) => {
                env.tables.element.insert(node, ElemRef::Base(e));
            }
            None => {
                env.tables.element.remove(node);
            }
        }

        let Some(element) = element else {
            env.tables.annotation_type.insert(node, TypeId::INVALID);
            if !should_ignore_undefined_named_type(&env.ctx, self.library_fragment, ast, node) {
                ErrorHelper::report_null_or_non_type_element(env, &self.source_path, ast, node, None);
            }
            return;
        };

        if element.tag() == Tag::MultiplyDefined {
            env.tables.annotation_type.insert(node, TypeId::INVALID);
            return;
        }

        let t = self.instantiate_element(env, ast, node, element);
        let t = self.verify_nullability(env, ast, node, t);
        env.tables.annotation_type.insert(node, t);
    }

    /// Dart `_rewriteToConstructorName`: `prefix.Name` where `prefix` is a
    /// class (or a type alias) is probably `Class.constructor`.
    fn rewrite_to_constructor_name(
        &mut self,
        env: &mut NamedTypeEnv<'_, '_>,
        ast: &mut Ast,
        node: Id<dartr_ast::NamedType>,
        import_prefix: Id<ImportPrefixReference>,
        import_prefix_element: ElementId,
        name_token: dartr_syntax::TokenId,
    ) {
        let constructor_name = ast.parent(node).and_then(|p| ast.cast::<ConstructorName>(p));
        if let Some(constructor_name) = constructor_name {
            if ast[constructor_name].name.is_none() {
                let prefix_token = ast[import_prefix].name;
                let period = ast[import_prefix].period;
                if let Some(type_arguments) = ast[node].type_arguments {
                    env.report(
                        diag::wrong_number_of_type_arguments_constructor(
                            ast.tokens.lexeme(prefix_token),
                            ast.tokens.lexeme(name_token),
                        )
                        .at_offset(
                            ast.offset(type_arguments) as usize,
                            ast.length(type_arguments) as usize,
                        ),
                    );
                    if let Some(instance_creation) = ast
                        .parent(constructor_name)
                        .and_then(|p| ast.cast::<InstanceCreationExpression>(p))
                    {
                        ast.modify(instance_creation, |n| n.type_arguments = Some(type_arguments));
                    }
                }

                let named_type = ast.add(dartr_ast::NamedType {
                    import_prefix: None,
                    name: prefix_token,
                    type_arguments: None,
                    question: None,
                });
                env.tables
                    .element
                    .insert(named_type, ElemRef::Base(import_prefix_element));
                let name = ast.add(SimpleIdentifier { token: name_token });
                ast.modify(constructor_name, |c| {
                    c.type_ = named_type;
                    c.period = Some(period);
                    c.name = Some(name);
                });
                self.rewrite_result = Some(constructor_name);
                return;
            }
        }

        env.tables.annotation_type.insert(node, TypeId::INVALID);
        if is_instance_creation(ast, node) {
            ErrorHelper::report_new_with_non_type(env, ast, node);
        } else {
            let ctx = env.ctx;
            let name = ast.tokens.lexeme(ast[node].name).to_string();
            let mut element = Some(import_prefix_element);
            if let Some(instance) = import_prefix_element.cast::<dartr_element::InstanceElement>() {
                let mut found: Option<ElementId> = None;
                if let Some(interface) = import_prefix_element.cast::<InterfaceElement>() {
                    found = dartr_typesystem::lookup::get_named_constructor(&ctx, interface, &name)
                        .map(|e| e.raw());
                }
                found = found
                    .or_else(|| crate::element_ext::get_field(&ctx, instance, &name).map(|e| e.raw()))
                    .or_else(|| dartr_typesystem::lookup::get_getter(&ctx, instance, &name).map(|e| e.raw()))
                    .or_else(|| dartr_typesystem::lookup::get_method(&ctx, instance, &name).map(|e| e.raw()))
                    .or_else(|| dartr_typesystem::lookup::get_setter(&ctx, instance, &name).map(|e| e.raw()));
                element = found;
            }
            let prefix_token = ast[import_prefix].name;
            let context = element.and_then(|e| declaration_context_message(&ctx, e, &name));
            let offset = ast.offset(import_prefix);
            env.report(
                diag::not_a_type(&format!(
                    "{}.{}",
                    ast.tokens.lexeme(prefix_token),
                    ast.tokens.lexeme(name_token)
                ))
                .with_context_messages(context)
                .at_offset(offset as usize, (token_end(ast, name_token) - offset) as usize),
            );
        }
    }

    /// Dart `_verifyNullability`.
    fn verify_nullability(
        &self,
        env: &mut NamedTypeEnv<'_, '_>,
        ast: &Ast,
        node: Id<dartr_ast::NamedType>,
        t: TypeId,
    ) -> TypeId {
        if self.class_hierarchy_named_type == Some(node)
            && env.ctx.nullability_suffix(t) == Nullability::Question
        {
            let parent = ast.parent(node);
            let at = |d: dartr_diagnostics::LocatableDiagnostic| {
                d.at_offset(ast.offset(node) as usize, ast.length(node) as usize)
            };
            if let Some(parent) = parent {
                if ast.is::<ExtendsClause>(parent) || ast.is::<ClassTypeAlias>(parent) {
                    env.report(at(diag::nullable_type_in_extends_clause()));
                } else if ast.is::<ImplementsClause>(parent) {
                    env.report(at(diag::nullable_type_in_implements_clause()));
                } else if ast.is::<MixinOnClause>(parent) {
                    env.report(at(diag::nullable_type_in_on_clause()));
                } else if ast.is::<WithClause>(parent) {
                    env.report(at(diag::nullable_type_in_with_clause()));
                }
            }
            return env.ctx.with_nullability(t, Nullability::None);
        }
        t
    }

    /// Dart `_verifyTypeAliasForContext`.
    fn verify_type_alias_for_context(
        &mut self,
        env: &mut NamedTypeEnv<'_, '_>,
        ast: &Ast,
        node: Id<dartr_ast::NamedType>,
        element: EId<TypeAliasElement>,
        t: TypeId,
    ) -> TypeId {
        let ctx = env.ctx;
        // If a type alias that expands to a type parameter.
        let aliased = ctx.get(element).aliased_type.get();
        if aliased.is_some_and(|a| matches!(ctx.ty(a), TypeKind::TypeParameter { .. })) {
            let parent = ast.parent(node);
            if let Some(constructor_name) = parent.and_then(|p| ast.cast::<ConstructorName>(p)) {
                let (offset, length) = ErrorHelper::error_range(env, ast, node, false);
                let constructor_usage = ast.parent(constructor_name);
                if constructor_usage.is_some_and(|u| ast.is::<InstanceCreationExpression>(u)) {
                    env.report(
                        diag::instantiate_type_alias_expands_to_type_parameter()
                            .at_offset(offset, length),
                    );
                } else if let Some(c) =
                    constructor_usage.and_then(|u| ast.cast::<ConstructorDeclaration>(u))
                {
                    if ast[c].redirected_constructor == Some(constructor_name) {
                        env.report(
                            diag::redirect_to_type_alias_expands_to_type_parameter()
                                .at_offset(offset, length),
                        );
                    }
                }
                // Dart throws `UnimplementedError` for other uses.
                return TypeId::INVALID;
            }

            // Report if this type is used as a class in hierarchy.
            let diagnostic = match parent {
                Some(p) if ast.is::<ExtendsClause>(p) => {
                    Some(diag::extends_type_alias_expands_to_type_parameter())
                }
                Some(p) if ast.is::<ImplementsClause>(p) => {
                    Some(diag::implements_type_alias_expands_to_type_parameter())
                }
                Some(p) if ast.is::<MixinOnClause>(p) => {
                    Some(diag::mixin_on_type_alias_expands_to_type_parameter())
                }
                Some(p) if ast.is::<WithClause>(p) => {
                    Some(diag::mixin_of_type_alias_expands_to_type_parameter())
                }
                _ => None,
            };
            if let Some(d) = diagnostic {
                let (offset, length) = ErrorHelper::error_range(env, ast, node, false);
                env.report(d.at_offset(offset, length));
                self.has_error_reported = true;
                return TypeId::INVALID;
            }
        }
        let is_interface = matches!(ctx.ty(t), TypeKind::Interface { .. });
        if !is_interface && is_instance_creation(ast, node) {
            ErrorHelper::report_new_with_non_type(env, ast, node);
            return TypeId::INVALID;
        }
        t
    }
}

/// Dart `NamedTypeResolver._isInstanceCreation`.
fn is_instance_creation(ast: &Ast, node: Id<dartr_ast::NamedType>) -> bool {
    ast.parent(node)
        .and_then(|p| ast.cast::<ConstructorName>(p))
        .and_then(|c| ast.parent(c))
        .is_some_and(|p| ast.is::<InstanceCreationExpression>(p))
}

/// The library fragment of [fragment] (Dart `fragment.libraryFragment`).
pub fn library_fragment_of(ctx: &Ctx<'_>, fragment: FragmentId) -> Option<FId<LibraryFragment>> {
    let mut current = Some(fragment);
    while let Some(f) = current {
        if let Some(l) = f.cast::<LibraryFragment>() {
            return Some(l);
        }
        current = ctx.fragment_data(f)?.enclosing_fragment;
    }
    None
}

/// "The declaration of '[name]' is here." at the first fragment of
/// [element] (when it has a source and a name offset).
fn declaration_context_message(
    ctx: &Ctx<'_>,
    element: ElementId,
    name: &str,
) -> Option<DiagnosticMessage> {
    let data = ctx.element_data(element)?;
    let fragment = ctx.fragment_data(data.first_fragment)?;
    let name_offset = fragment.name_offset?;
    let library_fragment = library_fragment_of(ctx, data.first_fragment)?;
    let source = &ctx.fragment(library_fragment).source;
    Some(DiagnosticMessage {
        file_path: source.path.to_string(),
        offset: name_offset as i64,
        length: name.encode_utf16().count() as i64,
        message: format!("The declaration of '{name}' is here."),
        url: None,
    })
}

/// Dart `LibraryFragmentImpl.shouldIgnoreUndefinedNamedType(node)`.
pub fn should_ignore_undefined_named_type(
    ctx: &Ctx<'_>,
    fragment: FId<LibraryFragment>,
    ast: &Ast,
    node: Id<dartr_ast::NamedType>,
) -> bool {
    let prefix = ast[node]
        .import_prefix
        .map(|p| ast.tokens.lexeme(ast[p].name));
    let name = ast.tokens.lexeme(ast[node].name);
    should_ignore_undefined(ctx, fragment, prefix, name)
}

/// Dart `LibraryFragmentImpl.shouldIgnoreUndefined(prefix:, name:)`.
pub fn should_ignore_undefined(
    ctx: &Ctx<'_>,
    fragment: FId<LibraryFragment>,
    prefix: Option<&str>,
    name: &str,
) -> bool {
    // Dart `withEnclosing`: this fragment and its enclosing fragments.
    let mut current = Some(fragment);
    while let Some(f) = current {
        let data = ctx.fragment(f);
        for import in &data.library_imports {
            let import_prefix = import
                .prefix
                .and_then(|p| ctx.fragment(p).element.try_get().copied())
                .and_then(|e| ctx.element_data(e))
                .and_then(|d| d.name)
                .map(|n| ctx.name_str(n));
            if import_prefix != prefix {
                continue;
            }
            let imported_library = match &import.directive.uri {
                dartr_element::DirectiveUri::Library { library, .. } => Some(*library),
                _ => None,
            };
            let not_existing = match imported_library {
                None => true,
                Some(l) => {
                    let first = ctx.get(l).first_fragment();
                    ctx.fragment(first)
                        .flags
                        .has(dartr_element::FragmentFlags::LIBRARY_FRAGMENT_IS_ORIGIN_NOT_EXISTING_FILE)
                }
            };
            if !not_existing {
                continue;
            }
            let show_combinators: Vec<&Vec<dartr_element::Name>> = import
                .combinators
                .iter()
                .filter_map(|c| match c {
                    dartr_element::NamespaceCombinator::Show { shown_names, .. } => {
                        Some(shown_names)
                    }
                    _ => None,
                })
                .collect();
            if prefix.is_some() && show_combinators.is_empty() {
                return true;
            }
            for shown in show_combinators {
                if shown.iter().any(|&n| ctx.name_str(n) == name) {
                    return true;
                }
            }
        }
        current = data
            .enclosing_fragment
            .and_then(|p| p.cast::<LibraryFragment>());
    }

    if prefix.is_none() && name.starts_with("_$") {
        for part in &ctx.fragment(fragment).parts {
            if let dartr_element::DirectiveUri::Source {
                relative_uri_string,
                ..
            } = &part.directive.uri
                && is_generated(relative_uri_string)
            {
                return true;
            }
        }
    }
    false
}

/// Dart `file_paths.isGenerated(path)`.
pub fn is_generated(path: &str) -> bool {
    const SUFFIXES: [&str; 6] = [
        ".g.dart",
        ".pb.dart",
        ".pbenum.dart",
        ".pbserver.dart",
        ".pbjson.dart",
        ".template.dart",
    ];
    SUFFIXES.iter().any(|s| path.ends_with(s))
}

/// Dart `_ErrorHelper`: diagnostics of type name resolution.
struct ErrorHelper;

impl ErrorHelper {
    /// Dart `reportNewWithNonType`.
    fn report_new_with_non_type(
        env: &mut NamedTypeEnv<'_, '_>,
        ast: &Ast,
        node: Id<dartr_ast::NamedType>,
    ) -> bool {
        let Some(constructor_name) = ast.parent(node).and_then(|p| ast.cast::<ConstructorName>(p))
        else {
            return false;
        };
        let Some(instance_creation) = ast
            .parent(constructor_name)
            .and_then(|p| ast.cast::<InstanceCreationExpression>(p))
        else {
            return false;
        };
        let (offset, length) = Self::error_range(env, ast, node, true);
        let import_prefix = ast[node].import_prefix;
        if let Some(import_prefix) =
            import_prefix.filter(|&p| env.tables.element.get(p).is_none())
        {
            // The constructor name is in two or three parts and the first
            // part, which is either a prefix or a class name, is
            // unresolved. Report that the first name is undefined.
            let prefix_or_class_name = ast.tokens.lexeme(ast[import_prefix].name);
            env.report(diag::undefined_identifier(prefix_or_class_name).at_offset(offset, length));
        } else {
            let class_name = ast.tokens.lexeme(ast[node].name);
            let d = if instance_creation_is_const(ast, instance_creation) {
                diag::const_with_non_type(class_name)
            } else {
                diag::new_with_non_type(class_name)
            };
            env.report(d.at_offset(offset, length));
        }
        true
    }

    /// Dart `reportNullOrNonTypeElement`.
    fn report_null_or_non_type_element(
        env: &mut NamedTypeEnv<'_, '_>,
        source_path: &str,
        ast: &Ast,
        node: Id<dartr_ast::NamedType>,
        element: Option<ElementId>,
    ) {
        let name_token = ast[node].name;
        if token_is_synthetic(ast, name_token) {
            return;
        }
        let name = ast.tokens.lexeme(name_token).to_string();

        if name == "boolean" {
            let (offset, length) = Self::error_range(env, ast, node, true);
            env.report(diag::undefined_class_boolean(&name).at_offset(offset, length));
            return;
        }

        let parent = ast.parent(node);

        if let Some(c) = parent.and_then(|p| ast.cast::<CatchClause>(p)) {
            if ast[c].exception_type.map(|t| t.raw()) == Some(node.raw()) {
                let (offset, length) = Self::error_range(env, ast, node, false);
                env.report(diag::non_type_in_catch_clause(&name).at_offset(offset, length));
                return;
            }
        }

        if let Some(a) = parent.and_then(|p| ast.cast::<AsExpression>(p)) {
            if ast[a].type_.raw() == node.raw() {
                let (offset, length) = Self::error_range(env, ast, node, false);
                env.report(diag::cast_to_non_type(&name).at_offset(offset, length));
                return;
            }
        }

        if let Some(i) = parent.and_then(|p| ast.cast::<IsExpression>(p)) {
            if ast[i].type_.raw() == node.raw() {
                let (offset, length) = Self::error_range(env, ast, node, false);
                let d = if element.is_some() {
                    diag::type_test_with_non_type(&name)
                } else {
                    diag::type_test_with_undefined_name(&name)
                };
                env.report(d.at_offset(offset, length));
                return;
            }
        }

        if Self::is_redirecting_constructor(ast, node) {
            let (offset, length) = Self::error_range(env, ast, node, false);
            env.report(diag::redirect_to_non_class(&name).at_offset(offset, length));
            return;
        }

        if parent.is_some_and(|p| ast.is::<TypeArgumentList>(p)) {
            let (offset, length) = Self::error_range(env, ast, node, false);
            env.report(diag::non_type_as_type_argument(&name).at_offset(offset, length));
            return;
        }

        if Self::report_new_with_non_type(env, ast, node) {
            return;
        }

        if parent.is_some_and(|p| {
            ast.is::<ExtendsClause>(p)
                || ast.is::<ImplementsClause>(p)
                || ast.is::<WithClause>(p)
                || ast.is::<ClassTypeAlias>(p)
        }) {
            // Ignored. The error will be reported elsewhere.
            return;
        }

        let ctx = env.ctx;
        if let Some(e) = element {
            if matches!(
                e.tag(),
                Tag::LocalVariable
                    | Tag::PatternVariable
                    | Tag::BindPatternVariable
                    | Tag::JoinPatternVariable
                    | Tag::LocalFunction
            ) {
                // Dart `DiagnosticFactory.referencedBeforeDeclaration`.
                let mut d = diag::referenced_before_declaration(&name);
                let declaration_offset = ctx
                    .element_data(e)
                    .and_then(|data| ctx.fragment_data(data.first_fragment))
                    .and_then(|f| f.name_offset);
                if let Some(declaration_offset) = declaration_offset {
                    d = d.with_context_messages([DiagnosticMessage {
                        file_path: source_path.to_string(),
                        offset: declaration_offset as i64,
                        length: name.encode_utf16().count() as i64,
                        message: format!("The declaration of '{name}' is here."),
                        url: None,
                    }]);
                }
                let offset = ast.tokens.offset(name_token);
                env.report(d.at_offset(offset as usize, (token_end(ast, name_token) - offset) as usize));
                return;
            }

            let (offset, length) = Self::error_range(env, ast, node, false);
            let context = declaration_context_message(&ctx, e, &name);
            env.report(
                diag::not_a_type(&name)
                    .with_context_messages(context)
                    .at_offset(offset, length),
            );
            return;
        }

        if ast[node].import_prefix.is_none() && name == "await" {
            env.report(
                diag::undefined_identifier_await()
                    .at_offset(ast.offset(node) as usize, ast.length(node) as usize),
            );
            return;
        }

        let (offset, length) = Self::error_range(env, ast, node, false);
        env.report(diag::undefined_class(&name).at_offset(offset, length));
    }

    /// Dart `_getErrorRange`: the range of the simple identifier of the
    /// (maybe prefixed) name, as (offset, length).
    fn error_range(
        env: &NamedTypeEnv<'_, '_>,
        ast: &Ast,
        node: Id<dartr_ast::NamedType>,
        skip_import_prefix: bool,
    ) -> (usize, usize) {
        let name = ast[node].name;
        let mut first_token = name;
        if let Some(import_prefix) = ast[node].import_prefix {
            let prefix_is_prefix_element = env
                .tables
                .element
                .get(import_prefix)
                .is_some_and(|e| matches!(e, ElemRef::Base(e) if e.tag() == Tag::Prefix));
            if !skip_import_prefix || !prefix_is_prefix_element {
                first_token = ast[import_prefix].name;
            }
        }
        let offset = ast.tokens.offset(first_token);
        let end = token_end(ast, name);
        (offset as usize, end.saturating_sub(offset) as usize)
    }

    /// Dart `_isRedirectingConstructor`.
    fn is_redirecting_constructor(ast: &Ast, node: Id<dartr_ast::NamedType>) -> bool {
        let Some(constructor_name) = ast.parent(node).and_then(|p| ast.cast::<ConstructorName>(p))
        else {
            return false;
        };
        ast.parent(constructor_name)
            .and_then(|p| ast.cast::<ConstructorDeclaration>(p))
            .is_some_and(|c| ast[c].redirected_constructor == Some(constructor_name))
    }
}

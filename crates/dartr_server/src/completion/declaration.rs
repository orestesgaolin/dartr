// Dart source: pkg/analysis_server/lib/src/services/completion/dart/declaration_helper.dart
// Dart source: pkg/analysis_server/lib/src/services/completion/dart/visibility_tracker.dart
// Dart source: pkg/analyzer/lib/src/dart/resolver/applicable_extensions.dart (applicableTo)

//! The declarations that are visible at the completion offset (Dart
//! `DeclarationHelper`).

use std::collections::{HashMap, HashSet};

use dartr_ast::*;
use dartr_element::{
    Ctx, EId, ElemRef, ElementId, ExtensionElement, InterfaceElement, LibraryElement, Nullability,
    Tag, TypeId, TypeKind,
};
use dartr_typesystem::inheritance_manager3::{InheritanceManager3, NameMap};
use dartr_typesystem::type_algebra::MapSubstitution;
use dartr_typesystem::{TypeExt, TypeSystem, member};

use super::candidate::{Candidate, ImportData, Kind, SuggestionKind, Typed, display_name, is_enum_constant};
use super::target::TokenExt;
use super::{Out, Request, elem};

/// Dart `VisibilityTracker`.
#[derive(Default)]
pub struct VisibilityTracker {
    declared_names: HashSet<String>,
    not_imported_names: HashMap<String, Vec<String>>,
}

impl VisibilityTracker {
    /// Dart `isVisible`.
    pub fn is_visible(&mut self, ctx: &Ctx<'_>, element: ElementId, import_data: Option<&ImportData>) -> bool {
        if element.tag() == Tag::Extension && ctx.element_name(element).is_none() {
            return false;
        }
        let name = display_name(ctx, element);
        let is_not_imported = import_data.is_some_and(|d| d.is_not_imported);
        let prefix = import_data.and_then(|d| d.prefix.as_deref());
        let qualified = match prefix {
            Some(p) => format!("{p}.{name}"),
            None => name.clone(),
        };
        if is_not_imported {
            if self.declared_names.contains(&qualified) {
                return false;
            }
            let uri = import_data.unwrap().library_uri.clone();
            let list = self.not_imported_names.entry(name).or_default();
            if list.is_empty() || !list.contains(&uri) {
                list.push(uri);
                return true;
            }
            return false;
        }
        self.declared_names.insert(qualified)
    }
}

/// A not-imported operation (Dart `NotImportedOperation`).
#[derive(Clone, Debug)]
pub enum NotImportedOp {
    Constructors,
    InstanceExtensionMembers {
        ty: TypeId,
        excluded_getters: Vec<String>,
        include_methods: bool,
        include_setters: bool,
    },
    StaticMembers,
}

/// The requirements of the suggestions (the fields of Dart
/// `DeclarationHelper`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeclConfig {
    pub must_be_assignable: bool,
    pub must_be_constant: bool,
    pub must_be_extendable: bool,
    pub must_be_implementable: bool,
    pub must_be_mixable: bool,
    pub must_be_non_void: bool,
    pub must_be_static: bool,
    pub must_be_type: bool,
    pub exclude_type_names: bool,
    pub object_pattern_allowed: bool,
    pub prefer_non_invocation: bool,
    pub suggesting_dot_shorthand: bool,
    pub suggest_unnamed_as_new: bool,
    pub skip_imports: bool,
    pub excluded_nodes: Vec<NodeId>,
}

/// Dart `DeclarationHelper`.
pub struct DeclarationHelper {
    pub cfg: DeclConfig,
    pub visibility: VisibilityTracker,
    variable_distance: i32,
    pub ops: Vec<NotImportedOp>,
}

/// The import namespace of an import (Dart `LibraryImport.namespace`):
/// the export namespace of the imported library after the combinators, in
/// the order of `NamespaceBuilder._applyCombinators`.
pub fn import_namespace(
    ctx: &Ctx<'_>,
    import: &dartr_element::LibraryImport,
) -> Option<(EId<LibraryElement>, Vec<(String, ElementId)>)> {
    let dartr_element::DirectiveUri::Library { library, .. } = &import.directive.uri else {
        return None;
    };
    let namespace = ctx.get(*library).export_namespace.try_get()?;
    let mut names: Vec<(String, ElementId)> = namespace
        .defined_names
        .iter()
        .map(|(n, e)| (ctx.name_str(*n).to_string(), *e))
        .collect();
    for combinator in &import.combinators {
        match combinator {
            dartr_element::NamespaceCombinator::Show { shown_names, .. } => {
                let mut result = Vec::new();
                for &n in shown_names {
                    let n = ctx.name_str(n);
                    for key in [n.to_string(), format!("{n}=")] {
                        if let Some(entry) = names.iter().find(|(k, _)| *k == key) {
                            if !result.iter().any(|(k, _): &(String, ElementId)| *k == key) {
                                result.push(entry.clone());
                            }
                        }
                    }
                }
                names = result;
            }
            dartr_element::NamespaceCombinator::Hide { hidden_names, .. } => {
                for &n in hidden_names {
                    let n = ctx.name_str(n);
                    names.retain(|(k, _)| k != n && *k != format!("{n}="));
                }
            }
        }
    }
    Some((*library, names))
}

/// The name of the prefix of an import.
fn import_prefix_name(ctx: &Ctx<'_>, import: &dartr_element::LibraryImport) -> Option<String> {
    let prefix = import.prefix?;
    let element = ctx.fragment(prefix).element.try_get().copied()?;
    ctx.element_name(element).map(str::to_string)
}

/// The prefix element of an import.
fn import_prefix_element(ctx: &Ctx<'_>, import: &dartr_element::LibraryImport) -> Option<ElementId> {
    let prefix = import.prefix?;
    ctx.fragment(prefix).element.try_get().copied()
}

/// Dart `isWildcard`.
fn is_wildcard(name: Option<&str>) -> bool {
    name == Some("_")
}

/// Dart `ExtensionsExtensions.applicableTo`: the extensions that apply to
/// [target_type] in [library].
pub fn applicable_extensions(
    ctx: &Ctx<'_>,
    library: EId<LibraryElement>,
    candidates: &[EId<ExtensionElement>],
    target_type: TypeId,
) -> Vec<EId<ExtensionElement>> {
    use dartr_typesystem::generic_inferrer::{GenericInferrer, InferenceFlags};
    use dartr_typesystem::type_algebra::get_fresh_type_parameters;
    use dartr_typesystem::type_system_operations::TypeSystemOperations;
    if matches!(*ctx.ty(target_type), TypeKind::Never(Nullability::None)) {
        return Vec::new();
    }
    let features = &ctx.get(library).feature_set;
    let flags = InferenceFlags {
        generic_metadata_is_enabled: features.is_enabled(
            dartr_parser::experimental_flags::ExperimentalFlag::GenericMetadata.name(),
        ),
        inference_using_bounds_is_enabled: features.is_enabled(
            dartr_parser::experimental_flags::ExperimentalFlag::InferenceUsingBounds.name(),
        ),
        strict_inference: false,
    };
    let ts = TypeSystem::new(*ctx);
    let mut result = Vec::new();
    for &e in candidates {
        let extension = ctx.get(e);
        let Some(extended) = extension.extended_type.get() else {
            continue;
        };
        let fresh = get_fresh_type_parameters(ctx, &extension.type_params);
        let raw = fresh.substitution.substitute_type(ctx, extended);
        let operations = TypeSystemOperations::new(ts, false);
        let inferred = {
            let mut inferrer = GenericInferrer::new(
                ts,
                &fresh.fresh_type_parameters,
                None,
                None,
                flags,
                operations,
                None,
            );
            inferrer.constrain_argument(target_type, raw, "extendedType", None);
            inferrer.try_choose_final_types(true)
        };
        let Some(inferred) = inferred else {
            continue;
        };
        let substitution = MapSubstitution::from_pairs(&extension.type_params, &inferred);
        let extended_type = substitution.substitute_type(ctx, extended);
        if !ts.is_subtype_of(target_type, extended_type) {
            continue;
        }
        result.push(e);
    }
    result
}

impl DeclarationHelper {
    pub fn new(cfg: DeclConfig) -> DeclarationHelper {
        DeclarationHelper {
            cfg,
            visibility: VisibilityTracker::default(),
            variable_distance: 0,
            ops: Vec::new(),
        }
    }

    fn is_dot_shorthand_enabled(&self, q: &Request<'_, '_>) -> bool {
        q.feature_enabled(dartr_parser::experimental_flags::ExperimentalFlag::DotShorthands)
    }

    /// Dart `_addTypeName`.
    fn add_type_name(&self, q: &Request<'_, '_>) -> bool {
        self.cfg.suggesting_dot_shorthand && !self.is_dot_shorthand_enabled(q)
    }

    /// Dart `_executableSuggestionKind`.
    fn executable_kind(&self) -> SuggestionKind {
        if self.cfg.prefer_non_invocation {
            SuggestionKind::Identifier
        } else {
            SuggestionKind::Invocation
        }
    }

    fn typed(&self, q: &Request<'_, '_>) -> Typed {
        Typed {
            add_type_name: self.add_type_name(q),
            replacement: q.replacement,
            ..Typed::default()
        }
    }

    fn visible(&mut self, q: &Request<'_, '_>, e: ElementId, import: Option<&ImportData>) -> bool {
        self.visibility.is_visible(q.ctx, e, import)
    }

    // ----------------------------------------------------------------- public

    /// Dart `addConstructorInvocations`.
    pub fn add_constructor_invocations(&mut self, q: &Request<'_, '_>, out: &mut Out) {
        let library = q.library;
        let import = ImportData {
            library_uri: elem::library_uri(q.ctx, library),
            prefix: None,
            is_not_imported: false,
        };
        self.add_constructors(q, out, library, Some(&import));
        if !self.cfg.skip_imports {
            self.add_imported_constructors(q, out);
            self.ops.push(NotImportedOp::Constructors);
        }
    }

    /// Dart `addConstructorNamesForElement`.
    pub fn add_constructor_names_for_element(
        &mut self,
        q: &Request<'_, '_>,
        out: &mut Out,
        element: EId<InterfaceElement>,
    ) {
        let constructors = q.ctx.interface(element).constructors.clone();
        for c in constructors {
            self.suggest_constructor(q, out, ElemRef::Base(c.raw()), None, true, false, None);
        }
    }

    /// Dart `addConstructorNamesForType`.
    pub fn add_constructor_names_for_type(
        &mut self,
        q: &Request<'_, '_>,
        out: &mut Out,
        ty: TypeId,
        exclude: Option<&str>,
    ) {
        let ctx = q.ctx;
        let Some(element) = ctx.interface_element(ty) else {
            return;
        };
        let substitution = MapSubstitution::from_interface_type(ctx, ty);
        let constructors = ctx.interface(element).constructors.clone();
        for c in constructors {
            let name = ctx.element_name(c.raw());
            if name != Some("new")
                && name != exclude
                && !(self.cfg.must_be_constant && !elem::is_const_constructor(ctx, c.raw()))
            {
                let r = member::substitute(ctx, ElemRef::Base(c.raw()), &substitution);
                self.suggest_constructor(q, out, r, None, true, false, None);
            }
        }
    }

    /// Dart `addDeclarationsThroughImportPrefix`.
    pub fn add_declarations_through_import_prefix(
        &mut self,
        q: &Request<'_, '_>,
        out: &mut Out,
        prefix: ElementId,
    ) {
        let ctx = q.ctx;
        for (fragment, _) in q.library_fragments() {
            for import in &ctx.fragment(fragment).library_imports {
                if import_prefix_element(ctx, import) != Some(prefix) {
                    continue;
                }
                let Some((library, names)) = import_namespace(ctx, import) else {
                    continue;
                };
                // Dart `_addDeclarationsImportedFrom(prefix: null)`.
                let import_data = ImportData {
                    library_uri: elem::library_uri(ctx, library),
                    prefix: None,
                    is_not_imported: false,
                };
                self.add_external_top_level_declarations(q, out, &names, &import_data);
                if let Some(pf) = import.prefix {
                    if ctx.fragment(pf).is_deferred {
                        let score = out.score("loadLibrary");
                        if score != -1.0 {
                            if let Some(&f) = ctx.get(library).load_library_function.try_get() {
                                out.add(Candidate::new(
                                    Kind::LoadLibrary { element: f.raw() },
                                    score,
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    /// Dart `addFieldsForInitializers`.
    pub fn add_fields_for_initializers(
        &mut self,
        q: &Request<'_, '_>,
        out: &mut Out,
        constructor: Id<ConstructorDeclaration>,
        field_to_include: Option<ElementId>,
    ) {
        let ctx = q.ctx;
        let ast = q.ast;
        let Some(constructor_element) = q.declared_element(constructor.raw()) else {
            return;
        };
        let Some(containing) = elem::enclosing(ctx, constructor_element) else {
            return;
        };
        let mut fields_to_skip: Vec<ElementId> = Vec::new();
        for &i in ast.list(ast[constructor].initializers) {
            if let Some(f) = ast.cast::<ConstructorFieldInitializer>(i.raw()) {
                if let Some(e) = q.element(ast[f].field_name.raw()) {
                    if e.tag() == Tag::Field {
                        fields_to_skip.push(e);
                    }
                }
            }
        }
        let parameters = ast[constructor].parameters;
        for &p in ast.list(ast[parameters].parameters) {
            if ast.is::<FieldFormalParameter>(p.raw()) {
                if let Some(e) = q.declared_element(p.raw()) {
                    if let Some(fp) = e.cast::<dartr_element::FormalParameterElement>() {
                        if let Some(field) = ctx.get(fp).field.get() {
                            fields_to_skip.push(field.raw());
                        }
                    }
                }
            }
        }
        fields_to_skip.retain(|f| Some(*f) != field_to_include);
        let Some(instance) = containing.cast::<dartr_element::InstanceElement>() else {
            return;
        };
        let fields = ctx.instance(instance).fields.clone();
        for field in fields {
            let f = field.raw();
            if !elem::is_static(ctx, f)
                && elem::is_origin_declaration(ctx, f)
                && !fields_to_skip.contains(&f)
                && (!(elem::is_final_variable(ctx, f) || elem::is_const_variable(ctx, f))
                    || !elem::has_initializer(ctx, f))
            {
                self.suggest_field(q, out, ElemRef::Base(f), None, false, false, false);
            }
        }
    }

    /// Dart `addFromLibrary`.
    pub fn add_from_library(
        &mut self,
        q: &Request<'_, '_>,
        out: &mut Out,
        library: EId<LibraryElement>,
        excluded: &[String],
    ) {
        let ctx = q.ctx;
        let Some(namespace) = ctx.get(library).export_namespace.try_get() else {
            return;
        };
        for (name, element) in &namespace.defined_names {
            if !excluded.iter().any(|e| e == ctx.name_str(*name)) {
                self.add_imported_element(q, out, *element);
            }
        }
    }

    /// Dart `addGetters`.
    pub fn add_getters(
        &mut self,
        q: &Request<'_, '_>,
        out: &mut Out,
        ty: TypeId,
        excluded: &[String],
        is_keyword_needed: bool,
        is_type_needed: bool,
    ) {
        match q.ctx.ty(ty) {
            TypeKind::Interface { .. } => self.add_instance_members(
                q,
                out,
                ty,
                excluded,
                true,
                false,
                is_keyword_needed,
                is_type_needed,
                false,
            ),
            TypeKind::Record { .. } => {
                self.add_fields_of_record_type(q, out, ty, excluded, is_keyword_needed, is_type_needed)
            }
            _ => {}
        }
    }

    /// Dart `addImportPrefixes`.
    pub fn add_import_prefixes(&mut self, q: &Request<'_, '_>, out: &mut Out) {
        let ctx = q.ctx;
        let first = ctx.get(q.library).first_fragment();
        for import in &ctx.fragment(first).library_imports {
            let Some(prefix) = import_prefix_element(ctx, import) else {
                continue;
            };
            if !self.visible(q, prefix, None) {
                continue;
            }
            let name = ctx.element_name(prefix);
            if name.is_none_or(str::is_empty) {
                continue;
            }
            if is_wildcard(name) && q.wildcard_variables() {
                continue;
            }
            let dartr_element::DirectiveUri::Library { library, .. } = &import.directive.uri else {
                continue;
            };
            let score = out.score(&display_name(ctx, prefix));
            if score != -1.0 {
                out.add(Candidate::new(
                    Kind::ImportPrefix {
                        library: library.raw(),
                        prefix,
                    },
                    score,
                ));
            }
        }
    }

    /// Dart `addInstanceMembersOfType`.
    pub fn add_instance_members_of_type(
        &mut self,
        q: &Request<'_, '_>,
        out: &mut Out,
        ty: TypeId,
        only_super: bool,
    ) {
        let ctx = q.ctx;
        let mut ty = ty;
        if let TypeKind::TypeParameter { .. } = ctx.ty(ty) {
            ty = ctx.type_parameter_type_bound(ty);
        }
        match ctx.ty(ty) {
            TypeKind::Interface { .. } => self.add_instance_members(
                q,
                out,
                ty,
                &[],
                !self.cfg.must_be_assignable,
                true,
                false,
                false,
                only_super,
            ),
            TypeKind::Record { .. } => {
                self.add_fields_of_record_type(q, out, ty, &[], false, false);
                self.add_members_of_dart_core_object(q, out);
                let include_methods = !self.cfg.must_be_assignable;
                self.add_extension_members(q, out, ty, &[], include_methods, true, false, false);
                self.ops.push(NotImportedOp::InstanceExtensionMembers {
                    ty,
                    excluded_getters: Vec::new(),
                    include_methods,
                    include_setters: true,
                });
            }
            TypeKind::Function(_) => {
                self.suggest_function_call(q, out, ty, None);
                self.add_members_of_dart_core_object(q, out);
            }
            TypeKind::Dynamic => self.add_members_of_dart_core_object(q, out),
            _ => {}
        }
    }

    /// Dart `addLexicalDeclarations`.
    pub fn add_lexical_declarations(&mut self, q: &Request<'_, '_>, out: &mut Out, node: NodeId) {
        let ast = q.ast;
        let containing = if self.cfg.must_be_type {
            self.add_local_types(q, out, node)
        } else {
            self.add_local_declarations(q, out, node)
        };
        let Some(containing) = containing else {
            return;
        };
        let mut parent = ast.parent(containing).or(Some(containing));
        if parent.is_some_and(|p| ast.is::<ClassNamePart>(p)) {
            parent = parent.and_then(|p| ast.parent(p));
        }
        if parent.is_some_and(|p| ast.is::<BlockClassBody>(p) || ast.is::<BlockEnumBody>(p)) {
            parent = parent.and_then(|p| ast.parent(p));
        }
        if let Some(p) = parent {
            if ast.is::<EnumConstantDeclaration>(p) || ast.is::<ClassMember>(p) || ast.is::<Directive>(p) {
                parent = ast.parent(p);
            } else if ast.is::<CompilationUnit>(p) {
                parent = Some(containing);
            }
        }
        if parent.is_some_and(|p| ast.is::<BlockClassBody>(p) || ast.is::<BlockEnumBody>(p)) {
            parent = parent.and_then(|p| ast.parent(p));
        }
        let mut top_level_member = None;
        if let Some(p) = parent.filter(|p| ast.is::<CompilationUnitMember>(*p)) {
            top_level_member = Some(p);
            self.add_members_of_enclosing_node(q, out, p);
            parent = ast.parent(p);
        }
        if parent.is_some_and(|p| ast.is::<CompilationUnit>(p)) {
            self.add_top_level_declarations(q, out);
            self.add_import_prefixes(q, out);
            if !self.cfg.skip_imports {
                self.add_imported_declarations(q, out);
            }
            self.ops.push(NotImportedOp::StaticMembers);
        }
        if let Some(t) = top_level_member {
            if !self.cfg.must_be_static && !self.cfg.must_be_type {
                self.add_inherited_members(q, out, t);
            }
        }
    }

    /// Dart `addMembersFromExtensionElement`.
    pub fn add_members_from_extension_element(
        &mut self,
        q: &Request<'_, '_>,
        out: &mut Out,
        extension: EId<ExtensionElement>,
        import_data: Option<&ImportData>,
        excluded: &[String],
        include_methods: bool,
        include_setters: bool,
    ) {
        let ctx = q.ctx;
        let data = ctx.get(extension);
        let referencing = data
            .extended_type
            .get()
            .and_then(|t| ctx.interface_element(t))
            .map(|e| e.raw());
        if include_methods {
            for &m in &data.methods {
                let m = m.raw();
                if elem::is_static(ctx, m)
                    || elem::is_operator(ctx, m)
                    || !elem::is_visible_in(ctx, m, q.library)
                {
                    continue;
                }
                self.suggest_method(q, out, ElemRef::Base(m), false, import_data, referencing, false, false);
            }
        }
        for &g in &data.getters {
            let g = g.raw();
            if excluded.iter().any(|e| Some(e.as_str()) == ctx.element_name(g))
                || elem::is_static(ctx, g)
                || !elem::is_visible_in(ctx, g, q.library)
            {
                continue;
            }
            self.suggest_property(q, out, ElemRef::Base(g), false, import_data, referencing, false, false, false);
        }
        if include_setters {
            for &s in &data.setters {
                let s = s.raw();
                if elem::is_static(ctx, s) || !elem::is_visible_in(ctx, s, q.library) {
                    continue;
                }
                self.suggest_property(q, out, ElemRef::Base(s), false, import_data, referencing, false, false, false);
            }
        }
    }

    /// Dart `addNotImportedConstructors`.
    pub fn add_not_imported_constructors(
        &mut self,
        q: &Request<'_, '_>,
        out: &mut Out,
        library: EId<LibraryElement>,
    ) {
        let import = ImportData {
            library_uri: elem::library_uri(q.ctx, library),
            prefix: None,
            is_not_imported: true,
        };
        self.add_constructors(q, out, library, Some(&import));
    }

    /// Dart `addNotImportedExtensionMethods`.
    pub fn add_not_imported_extension_methods(
        &mut self,
        q: &Request<'_, '_>,
        out: &mut Out,
        library: EId<LibraryElement>,
        ty: TypeId,
        excluded: &[String],
        include_methods: bool,
        include_setters: bool,
    ) {
        let ctx = q.ctx;
        let Some(namespace) = ctx.get(library).export_namespace.try_get() else {
            return;
        };
        let extensions: Vec<EId<ExtensionElement>> = namespace
            .defined_names
            .values()
            .filter_map(|e| e.cast::<ExtensionElement>())
            .collect();
        let ts = TypeSystem::new(*ctx);
        let target = if ctx.is_dart_core_null(ty) {
            ty
        } else {
            ts.promote_to_non_null(ty)
        };
        let applicable = applicable_extensions(ctx, library, &extensions, target);
        let import = ImportData {
            library_uri: elem::library_uri(ctx, library),
            prefix: None,
            is_not_imported: true,
        };
        for e in applicable {
            if elem::is_visible_in(ctx, e.raw(), q.library) {
                self.add_members_from_extension_element(
                    q,
                    out,
                    e,
                    Some(&import),
                    excluded,
                    include_methods,
                    include_setters,
                );
            }
        }
    }

    /// Dart `addNotImportedTopLevelDeclarations`.
    pub fn add_not_imported_top_level_declarations(
        &mut self,
        q: &Request<'_, '_>,
        out: &mut Out,
        library: EId<LibraryElement>,
    ) {
        let ctx = q.ctx;
        let import = ImportData {
            library_uri: elem::library_uri(ctx, library),
            prefix: None,
            is_not_imported: true,
        };
        let Some(namespace) = ctx.get(library).export_namespace.try_get() else {
            return;
        };
        let names: Vec<(String, ElementId)> = namespace
            .defined_names
            .iter()
            .map(|(n, e)| (ctx.name_str(*n).to_string(), *e))
            .collect();
        self.add_external_top_level_declarations(q, out, &names, &import);
    }

    /// Dart `addParametersFromSuperConstructor`.
    pub fn add_parameters_from_super_constructor(
        &mut self,
        q: &Request<'_, '_>,
        out: &mut Out,
        node: Id<SuperFormalParameter>,
    ) {
        let ctx = q.ctx;
        let ast = q.ast;
        let Some(element) = q.declared_element(node.raw()) else {
            return;
        };
        if element.tag() != Tag::SuperFormalParameter {
            return;
        }
        let Some(constructor) = ast.this_or_ancestor_of_type::<ConstructorDeclaration>(node) else {
            return;
        };
        let Some(constructor_element) = q.declared_element(constructor.raw()) else {
            return;
        };
        let Some(c) = constructor_element.cast::<dartr_element::ConstructorElement>() else {
            return;
        };
        let Some(super_constructor) = ctx.get(c).super_constructor.get() else {
            return;
        };
        let super_parameters = member::formal_parameters(ctx, super_constructor);
        let kind = ast[node].kind;
        if kind.is_named() {
            let mut specified: Vec<String> = ctx
                .get(c)
                .formal_params
                .iter()
                .filter_map(|p| ctx.element_name(p.raw()).map(str::to_string))
                .collect();
            let invocations: Vec<_> = ast
                .list(ast[constructor].initializers)
                .iter()
                .filter_map(|i| ast.cast::<SuperConstructorInvocation>(i.raw()))
                .collect();
            if invocations.len() == 1 {
                let list = ast[invocations[0]].argument_list;
                for &a in ast.list_raw(ast[list].arguments) {
                    if let Some(n) = ast.cast::<NamedArgument>(a) {
                        specified.push(ast.t_lexeme(ast[n].name).to_string());
                    }
                }
            }
            for p in super_parameters {
                let base = member::base_element(ctx, p);
                if elem::parameter_kind(ctx, p).is_named()
                    && !ctx
                        .element_name(base)
                        .is_some_and(|n| specified.iter().any(|s| s == n))
                {
                    self.suggest_super_parameter(out, ctx, p);
                }
            }
        } else if kind.is_positional() {
            let index = ctx
                .get(c)
                .formal_params
                .iter()
                .position(|p| p.raw() == element);
            let positional: Vec<ElemRef> = super_parameters
                .into_iter()
                .filter(|p| elem::parameter_kind(ctx, *p).is_positional())
                .collect();
            if let Some(i) = index {
                if let Some(p) = positional.get(i) {
                    self.suggest_super_parameter(out, ctx, *p);
                }
            }
        }
    }

    /// Dart `addPossibleRedirectionsInLibrary`.
    pub fn add_possible_redirections_in_library(
        &mut self,
        q: &Request<'_, '_>,
        out: &mut Out,
        redirecting: ElementId,
        library: EId<LibraryElement>,
    ) {
        let ctx = q.ctx;
        let Some(class) = elem::enclosing(ctx, redirecting).and_then(|c| c.cast::<InterfaceElement>())
        else {
            return;
        };
        let class_type = ctx.interface_this_type(class);
        let ts = TypeSystem::new(*ctx);
        for &c in &ctx.get(library).classes.clone() {
            let interface = c.raw().cast::<InterfaceElement>().unwrap();
            if ts.is_subtype_of(ctx.interface_this_type(interface), class_type) {
                for constructor in ctx.interface(interface).constructors.clone() {
                    let constructor = constructor.raw();
                    if constructor != redirecting && elem::is_accessible_in(ctx, constructor, library) {
                        self.suggest_constructor(q, out, ElemRef::Base(constructor), None, false, true, None);
                    }
                }
            }
        }
    }

    /// Dart `addStaticMembersOfElement`.
    pub fn add_static_members_of_element(
        &mut self,
        q: &Request<'_, '_>,
        out: &mut Out,
        element: ElementId,
        only_invocations: bool,
    ) {
        let ctx = q.ctx;
        let mut element = element;
        if let Some(alias) = element.cast::<dartr_element::TypeAliasElement>() {
            if let Some(t) = ctx.get(alias).aliased_type.get() {
                if let Some(e) = ctx.interface_element(t) {
                    element = e.raw();
                }
            }
        }
        let Some(instance) = element.cast::<dartr_element::InstanceElement>() else {
            return;
        };
        let data = ctx.instance(instance);
        let constructors: Vec<ElementId> = match element.cast::<InterfaceElement>() {
            Some(i) => ctx.interface(i).constructors.iter().map(|c| c.raw()).collect(),
            None => Vec::new(),
        };
        let getters: Vec<ElementId> = data.getters.iter().map(|g| g.raw()).collect();
        let setters: Vec<ElementId> = data.setters.iter().map(|g| g.raw()).collect();
        let fields: Vec<ElementId> = data.fields.iter().map(|g| g.raw()).collect();
        let methods: Vec<ElementId> = data.methods.iter().map(|g| g.raw()).collect();
        self.add_static_members(
            q, out, &getters, &setters, &constructors, element, &fields, &methods, only_invocations,
        );
    }

    // ---------------------------------------------------------------- private

    fn add_constructors(
        &mut self,
        q: &Request<'_, '_>,
        out: &mut Out,
        library: EId<LibraryElement>,
        import: Option<&ImportData>,
    ) {
        let ctx = q.ctx;
        let l = ctx.get(library);
        for &c in &l.classes {
            let e = c.raw();
            let constructors: Vec<ElementId> = ctx
                .interface(e.cast().unwrap())
                .constructors
                .iter()
                .map(|c| c.raw())
                .collect();
            self.suggest_constructors(q, out, &constructors, import, !elem::is_abstract_class(ctx, e), None);
        }
        for &c in &l.enums {
            let constructors: Vec<ElementId> = ctx
                .interface(c.raw().cast().unwrap())
                .constructors
                .iter()
                .map(|c| c.raw())
                .collect();
            self.suggest_constructors(q, out, &constructors, import, true, None);
        }
        for &c in &l.extension_types {
            let constructors: Vec<ElementId> = ctx
                .interface(c.raw().cast().unwrap())
                .constructors
                .iter()
                .map(|c| c.raw())
                .collect();
            self.suggest_constructors(q, out, &constructors, import, true, None);
        }
        for &a in &l.type_aliases {
            self.add_constructors_for_aliased_element(q, out, a.raw(), import);
        }
    }

    fn add_constructors_for_aliased_element(
        &mut self,
        q: &Request<'_, '_>,
        out: &mut Out,
        alias: ElementId,
        import: Option<&ImportData>,
    ) {
        let ctx = q.ctx;
        let Some(a) = alias.cast::<dartr_element::TypeAliasElement>() else {
            return;
        };
        let Some(t) = ctx.get(a).aliased_type.get() else {
            return;
        };
        let Some(aliased) = ctx.interface_element(t) else {
            return;
        };
        let e = aliased.raw();
        let constructors: Vec<ElementId> = ctx
            .interface(aliased)
            .constructors
            .iter()
            .map(|c| c.raw())
            .collect();
        match e.tag() {
            Tag::Class => self.suggest_constructors(
                q,
                out,
                &constructors,
                import,
                !elem::is_abstract_class(ctx, e),
                Some(alias),
            ),
            Tag::ExtensionType | Tag::Mixin => {
                self.suggest_constructors(q, out, &constructors, import, true, Some(alias))
            }
            _ => {}
        }
    }

    fn add_constructors_imported_from(
        &mut self,
        q: &Request<'_, '_>,
        out: &mut Out,
        library: EId<LibraryElement>,
        names: &[(String, ElementId)],
        prefix: Option<String>,
    ) {
        let ctx = q.ctx;
        let import = ImportData {
            library_uri: elem::library_uri(ctx, library),
            prefix,
            is_not_imported: false,
        };
        for &(_, element) in names {
            if !self.visible(q, element, Some(&import)) {
                continue;
            }
            match element.tag() {
                Tag::Class => {
                    let constructors: Vec<ElementId> = ctx
                        .interface(element.cast().unwrap())
                        .constructors
                        .iter()
                        .map(|c| c.raw())
                        .collect();
                    self.suggest_constructors(
                        q,
                        out,
                        &constructors,
                        Some(&import),
                        !elem::is_abstract_class(ctx, element),
                        None,
                    );
                }
                Tag::ExtensionType => {
                    let constructors: Vec<ElementId> = ctx
                        .interface(element.cast().unwrap())
                        .constructors
                        .iter()
                        .map(|c| c.raw())
                        .collect();
                    self.suggest_constructors(q, out, &constructors, Some(&import), true, None);
                }
                Tag::TypeAlias => {
                    self.add_constructors_for_aliased_element(q, out, element, Some(&import))
                }
                _ => {}
            }
        }
    }

    /// Dart `_addExtensionMembers`.
    #[allow(clippy::too_many_arguments)]
    fn add_extension_members(
        &mut self,
        q: &Request<'_, '_>,
        out: &mut Out,
        ty: TypeId,
        excluded: &[String],
        include_methods: bool,
        include_setters: bool,
        is_keyword_needed: bool,
        is_type_needed: bool,
    ) {
        let ctx = q.ctx;
        let ts = TypeSystem::new(*ctx);
        let target = if ctx.is_dart_core_null(ty) {
            ty
        } else {
            ts.promote_to_non_null(ty)
        };
        let accessible = q.accessible_extensions();
        let applicable = applicable_extensions(ctx, q.library, &accessible, target);
        let library = q.library;
        for e in applicable {
            let data = ctx.get(e);
            if include_methods {
                for &m in &data.methods {
                    let m = m.raw();
                    if elem::is_static(ctx, m) || elem::is_operator(ctx, m) || !elem::is_visible_in(ctx, m, library) {
                        continue;
                    }
                    self.suggest_method(q, out, ElemRef::Base(m), false, None, None, is_keyword_needed, is_type_needed);
                }
            }
            for &g in &data.getters {
                let g = g.raw();
                if excluded.iter().any(|x| Some(x.as_str()) == ctx.element_name(g)) {
                    continue;
                }
                if elem::is_origin_declaration(ctx, g) {
                    if elem::is_visible_in(ctx, g, library) {
                        self.suggest_property(q, out, ElemRef::Base(g), false, None, None, false, is_keyword_needed, is_type_needed);
                    }
                } else if let Some(v) = elem::accessor_variable(ctx, g) {
                    if v.tag() == Tag::Field && elem::is_visible_in(ctx, v, library) {
                        self.suggest_field(q, out, ElemRef::Base(v), None, false, is_keyword_needed, is_type_needed);
                    }
                }
            }
            if include_setters {
                for &s in &data.setters {
                    let s = s.raw();
                    if elem::is_origin_variable(ctx, s) || !elem::is_visible_in(ctx, s, library) {
                        continue;
                    }
                    self.suggest_property(q, out, ElemRef::Base(s), false, None, None, false, false, false);
                }
            }
        }
    }

    /// Dart `_addExternalTopLevelDeclarations`.
    fn add_external_top_level_declarations(
        &mut self,
        q: &Request<'_, '_>,
        out: &mut Out,
        names: &[(String, ElementId)],
        import: &ImportData,
    ) {
        let ctx = q.ctx;
        let must_be_type = self.cfg.must_be_type;
        for &(_, element) in names {
            match element.tag() {
                Tag::Class => self.suggest_class(q, out, element, Some(import)),
                Tag::Enum => self.suggest_enum(q, out, element, Some(import)),
                Tag::Extension => {
                    if !must_be_type {
                        self.suggest_extension(q, out, element, Some(import));
                    }
                }
                Tag::ExtensionType => self.suggest_extension_type(q, out, element, Some(import)),
                Tag::TopLevelFunction => {
                    if !must_be_type {
                        self.suggest_top_level_function(q, out, element, Some(import));
                    }
                }
                Tag::Mixin => self.suggest_mixin(q, out, element, Some(import)),
                Tag::Getter => {
                    if !must_be_type {
                        self.suggest_top_level_property(q, out, element, Some(import));
                    }
                }
                Tag::Setter => {
                    if !must_be_type && !elem::is_origin_variable(ctx, element) {
                        self.suggest_top_level_property(q, out, element, Some(import));
                    }
                }
                Tag::TopLevelVariable => {
                    if !must_be_type {
                        self.suggest_top_level_variable(q, out, element, Some(import));
                    }
                }
                Tag::TypeAlias => self.suggest_type_alias(q, out, element, Some(import)),
                _ => {}
            }
        }
    }

    /// Dart `_addFieldsOfRecordType`.
    fn add_fields_of_record_type(
        &mut self,
        q: &Request<'_, '_>,
        out: &mut Out,
        ty: TypeId,
        excluded: &[String],
        is_keyword_needed: bool,
        is_type_needed: bool,
    ) {
        let ctx = q.ctx;
        let TypeKind::Record {
            positional, named, ..
        } = *ctx.ty(ty)
        else {
            return;
        };
        for (index, &field) in ctx.list(positional).iter().enumerate() {
            self.suggest_record_field(q, out, field, format!("${}", index + 1), false, false);
        }
        for field in ctx.list(named) {
            let name = ctx.name_str(field.name).to_string();
            if !excluded.contains(&name) {
                self.suggest_record_field(q, out, field.ty, name, is_keyword_needed, is_type_needed);
            }
        }
    }

    /// Dart `_addImportedConstructors`.
    fn add_imported_constructors(&mut self, q: &Request<'_, '_>, out: &mut Out) {
        let ctx = q.ctx;
        let first = ctx.get(q.library).first_fragment();
        for import in &ctx.fragment(first).library_imports {
            if let Some((library, names)) = import_namespace(ctx, import) {
                let prefix = import_prefix_name(ctx, import);
                self.add_constructors_imported_from(q, out, library, &names, prefix);
            }
        }
    }

    /// Dart `_addImportedDeclarations`.
    fn add_imported_declarations(&mut self, q: &Request<'_, '_>, out: &mut Out) {
        let ctx = q.ctx;
        let first = ctx.get(q.library).first_fragment();
        for import in &ctx.fragment(first).library_imports {
            let Some((library, names)) = import_namespace(ctx, import) else {
                continue;
            };
            let prefix = import_prefix_name(ctx, import);
            if prefix.as_deref() != Some("_") {
                let import_data = ImportData {
                    library_uri: elem::library_uri(ctx, library),
                    prefix,
                    is_not_imported: false,
                };
                self.add_external_top_level_declarations(q, out, &names, &import_data);
            }
            if elem::library_uri(ctx, library) == "dart:core" && self.cfg.must_be_type {
                let score = out.score("Never");
                if score != -1.0 {
                    out.add(Candidate::new(Kind::Name("Never".to_string()), score));
                }
            }
        }
    }

    /// Dart `_addImportedElement`.
    fn add_imported_element(&mut self, q: &Request<'_, '_>, out: &mut Out, element: ElementId) {
        let ctx = q.ctx;
        let score = out.score(&display_name(ctx, element));
        if score == -1.0 {
            return;
        }
        let kind = match element.tag() {
            Tag::Class => Some(Kind::Class(element)),
            Tag::Enum => Some(Kind::Enum(element)),
            Tag::Extension => Some(Kind::Extension {
                element,
                kind: SuggestionKind::Invocation,
            }),
            Tag::ExtensionType => Some(Kind::ExtensionType(element)),
            Tag::TopLevelFunction => Some(Kind::TopLevelFunction {
                element,
                kind: self.executable_kind(),
            }),
            Tag::Mixin => Some(Kind::Mixin(element)),
            Tag::Getter | Tag::Setter => self.top_level_property_kind(ctx, element),
            Tag::TopLevelVariable => Some(Kind::TopLevelVariable(element)),
            Tag::TypeAlias => Some(Kind::TypeAlias(element)),
            _ => None,
        };
        if let Some(kind) = kind {
            out.add(Candidate::new(kind, score));
        }
    }

    /// Dart `_addInheritedMembers`.
    fn add_inherited_members(&mut self, q: &Request<'_, '_>, out: &mut Out, member_node: NodeId) {
        let ctx = q.ctx;
        let ast = q.ast;
        let is_supported = ast.is::<ClassDeclaration>(member_node)
            || ast.is::<EnumDeclaration>(member_node)
            || ast.is::<ExtensionDeclaration>(member_node)
            || ast.is::<ExtensionTypeDeclaration>(member_node)
            || ast.is::<MixinDeclaration>(member_node)
            || ast.is::<ClassTypeAlias>(member_node)
            || ast.is::<GenericTypeAlias>(member_node);
        if !is_supported {
            return;
        }
        let Some(element) = q.declared_element(member_node) else {
            return;
        };
        if !self.cfg.must_be_static {
            if let Some(extension) = element.cast::<ExtensionElement>() {
                if let Some(t) = ctx.get(extension).extended_type.get() {
                    if matches!(ctx.ty(t), TypeKind::Interface { .. }) {
                        self.add_instance_members(q, out, t, &[], true, true, false, false, false);
                    }
                }
                return;
            }
        }
        let Some(interface) = element.cast::<InterfaceElement>() else {
            return;
        };
        let referencing = Some(element);
        let im = InheritanceManager3::new(*ctx);
        let members: Vec<ElemRef> = im.get_inherited_map(interface).values().copied().collect();
        for m in members {
            let base = member::base_element(ctx, m);
            if !elem::is_visible_in(ctx, base, q.library) {
                continue;
            }
            match base.tag() {
                Tag::Method => {
                    if elem::is_operator(ctx, base) {
                        continue;
                    }
                    self.suggest_method(q, out, m, false, None, referencing, false, false);
                }
                Tag::Getter | Tag::Setter => {
                    self.suggest_property(q, out, m, false, None, referencing, false, false, false);
                }
                _ => {}
            }
        }
    }

    /// Dart `_addInstanceMembers`.
    #[allow(clippy::too_many_arguments)]
    fn add_instance_members(
        &mut self,
        q: &Request<'_, '_>,
        out: &mut Out,
        ty: TypeId,
        excluded: &[String],
        include_methods: bool,
        include_setters: bool,
        is_keyword_needed: bool,
        is_type_needed: bool,
        only_super: bool,
    ) {
        let ctx = q.ctx;
        let Some(element) = ctx.interface_element(ty) else {
            return;
        };
        let im = InheritanceManager3::new(*ctx);
        let map: NameMap = if only_super {
            im.get_inherited_concrete_map(element)
        } else {
            im.get_interface(element).map.clone()
        };
        let substitution = MapSubstitution::from_interface_type(ctx, ty);
        let mut by_name: indexmap::IndexMap<String, Vec<ElemRef>> = indexmap::IndexMap::new();
        for m in map.values() {
            let m = member::substitute(ctx, *m, &substitution);
            if self.can_access_instance_member(q, m) {
                let base = member::base_element(ctx, m);
                by_name.entry(display_name(ctx, base)).or_default().push(m);
            }
        }
        let referencing = Some(element.raw());
        for (name, members) in by_name {
            let m = self.best_member(ctx, &members);
            let base = member::base_element(ctx, m);
            match base.tag() {
                Tag::Method => {
                    if include_methods {
                        if elem::is_operator(ctx, base) {
                            continue;
                        }
                        self.suggest_method(q, out, m, false, None, referencing, is_keyword_needed, is_type_needed);
                    }
                }
                Tag::Getter => {
                    if !excluded.contains(&name) {
                        self.suggest_property(q, out, m, false, None, referencing, false, is_keyword_needed, is_type_needed);
                    }
                }
                Tag::Setter => {
                    if include_setters {
                        self.suggest_property(q, out, m, false, None, referencing, false, false, false);
                    }
                }
                _ => {}
            }
        }
        let all_supertypes = ctx.element_all_supertypes(element);
        let is_function = ctx.is_dart_core_function(ty);
        if (is_function && !only_super) || all_supertypes.iter().any(|t| ctx.is_dart_core_function(*t)) {
            let d = TypeId::DYNAMIC;
            let function_type = ctx.function_type(&[], &[], d, Nullability::None, None);
            self.suggest_function_call(q, out, function_type, None);
        }
        self.add_extension_members(q, out, ty, excluded, include_methods, include_setters, is_keyword_needed, is_type_needed);
        self.ops.push(NotImportedOp::InstanceExtensionMembers {
            ty,
            excluded_getters: excluded.to_vec(),
            include_methods,
            include_setters,
        });
    }

    /// Dart `_addLocalDeclarations`.
    fn add_local_declarations(&mut self, q: &Request<'_, '_>, out: &mut Out, node: NodeId) -> Option<NodeId> {
        let ast = q.ast;
        let mut previous: Option<NodeId> = None;
        let mut current = Some(node);
        while let Some(c) = current {
            match ast.kind(c) {
                NodeKind::Block => {
                    let b = ast.cast::<Block>(c).unwrap();
                    let statements: Vec<NodeId> = ast.list_raw(ast[b].statements).to_vec();
                    self.visit_statements(q, out, &statements, previous);
                }
                NodeKind::CatchClause => self.visit_catch_clause(q, out, c),
                NodeKind::CommentReference => return self.visit_comment_reference(q, out, c),
                NodeKind::ConstructorDeclaration => {
                    let d = ast.cast::<ConstructorDeclaration>(c).unwrap();
                    self.visit_parameter_list(q, out, Some(ast[d].parameters));
                    return Some(c);
                }
                NodeKind::DeclaredVariablePattern => self.visit_declared_variable_pattern(q, out, c),
                NodeKind::FieldDeclaration => return Some(c),
                NodeKind::ForElement => {
                    let f = ast.cast::<ForElement>(c).unwrap();
                    let parts = ast[f].for_loop_parts.raw();
                    if Some(parts) != previous {
                        self.visit_for_loop_parts(q, out, parts);
                    }
                }
                NodeKind::ForStatement => {
                    let f = ast.cast::<ForStatement>(c).unwrap();
                    let parts = ast[f].for_loop_parts.raw();
                    if Some(parts) != previous {
                        self.visit_for_loop_parts(q, out, parts);
                    }
                }
                NodeKind::ForPartsWithDeclarations => {
                    let f = ast.cast::<ForPartsWithDeclarations>(c).unwrap();
                    if Some(ast[f].variables.raw()) != previous {
                        self.visit_for_loop_parts(q, out, c);
                    }
                }
                NodeKind::FunctionDeclaration => {
                    let parent = ast.parent(c);
                    if !parent.is_some_and(|p| ast.is::<FunctionDeclarationStatement>(p)) {
                        return Some(c);
                    }
                }
                NodeKind::FunctionDeclarationStatement => {
                    let s = ast.cast::<FunctionDeclarationStatement>(c).unwrap();
                    let declaration = ast[s].function_declaration;
                    if let Some(e) = q.declared_element(declaration.raw()) {
                        self.suggest_local_function(q, out, e);
                    }
                }
                NodeKind::FunctionExpression => {
                    let f = ast.cast::<FunctionExpression>(c).unwrap();
                    self.visit_parameter_list(q, out, ast[f].parameters);
                    self.visit_type_parameter_list(q, out, ast[f].type_parameters);
                }
                NodeKind::GuardedPattern => {
                    let g = ast.cast::<GuardedPattern>(c).unwrap();
                    self.visit_pattern(q, out, ast[g].pattern.raw());
                }
                NodeKind::IfElement => {
                    let n = ast.cast::<IfElement>(c).unwrap();
                    if ast[n].else_keyword.is_none_or(|e| q.offset < ast.t_offset(e)) {
                        if let Some(cc) = ast[n].case_clause {
                            let pattern = ast[ast[cc].guarded_pattern].pattern;
                            self.visit_pattern(q, out, pattern.raw());
                        }
                    }
                }
                NodeKind::IfStatement => {
                    let n = ast.cast::<IfStatement>(c).unwrap();
                    if ast[n].else_keyword.is_none_or(|e| q.offset < ast.t_offset(e)) {
                        if let Some(cc) = ast[n].case_clause {
                            let pattern = ast[ast[cc].guarded_pattern].pattern;
                            self.visit_pattern(q, out, pattern.raw());
                        }
                    }
                }
                NodeKind::MethodDeclaration => {
                    let m = ast.cast::<MethodDeclaration>(c).unwrap();
                    self.visit_parameter_list(q, out, ast[m].parameters);
                    self.visit_type_parameter_list(q, out, ast[m].type_parameters);
                    return Some(c);
                }
                NodeKind::SwitchCase => {
                    let s = ast.cast::<SwitchCase>(c).unwrap();
                    let statements: Vec<NodeId> = ast.list_raw(ast[s].statements).to_vec();
                    self.visit_statements(q, out, &statements, previous);
                }
                NodeKind::SwitchDefault => {
                    let s = ast.cast::<SwitchDefault>(c).unwrap();
                    let statements: Vec<NodeId> = ast.list_raw(ast[s].statements).to_vec();
                    self.visit_statements(q, out, &statements, previous);
                }
                NodeKind::SwitchExpressionCase => {
                    let s = ast.cast::<SwitchExpressionCase>(c).unwrap();
                    if q.offset >= ast.t_end(ast[s].arrow) {
                        let pattern = ast[ast[s].guarded_pattern].pattern;
                        self.visit_pattern(q, out, pattern.raw());
                    }
                }
                NodeKind::SwitchPatternCase => self.visit_switch_pattern_case(q, out, c, previous),
                NodeKind::VariableDeclarationList => {
                    let l = ast.cast::<VariableDeclarationList>(c).unwrap();
                    if let Some(child) = previous.and_then(|p| ast.cast::<VariableDeclaration>(p)) {
                        let variables = ast.list(ast[l].variables);
                        if let Some(index) = variables.iter().position(|v| *v == child) {
                            for i in (0..index).rev() {
                                if let Some(e) = q.declared_element(variables[i].raw()) {
                                    if is_local_variable(e) {
                                        self.suggest_variable(q, out, e);
                                    }
                                }
                            }
                        }
                    }
                }
                NodeKind::CompilationUnit => return Some(c),
                _ if ast.is::<CompilationUnitMember>(c) => return Some(c),
                _ => {}
            }
            previous = Some(c);
            current = ast.parent(c);
        }
        current
    }

    /// Dart `_addLocalTypes`.
    fn add_local_types(&mut self, q: &Request<'_, '_>, out: &mut Out, node: NodeId) -> Option<NodeId> {
        let ast = q.ast;
        let mut current = Some(node);
        while let Some(c) = current {
            match ast.kind(c) {
                NodeKind::CommentReference
                | NodeKind::ConstructorDeclaration
                | NodeKind::FieldDeclaration
                | NodeKind::CompilationUnit => return Some(c),
                NodeKind::FunctionDeclaration => {
                    if !ast
                        .parent(c)
                        .is_some_and(|p| ast.is::<FunctionDeclarationStatement>(p))
                    {
                        return Some(c);
                    }
                }
                NodeKind::FunctionExpression => {
                    let f = ast.cast::<FunctionExpression>(c).unwrap();
                    self.visit_type_parameter_list(q, out, ast[f].type_parameters);
                }
                NodeKind::GenericFunctionType => {
                    let f = ast.cast::<GenericFunctionType>(c).unwrap();
                    self.visit_type_parameter_list(q, out, ast[f].type_parameters);
                }
                NodeKind::MethodDeclaration => {
                    let m = ast.cast::<MethodDeclaration>(c).unwrap();
                    self.visit_type_parameter_list(q, out, ast[m].type_parameters);
                    return Some(c);
                }
                _ if ast.is::<CompilationUnitMember>(c) => return Some(c),
                _ => {}
            }
            current = ast.parent(c);
        }
        current
    }

    fn add_members_of_dart_core_object(&mut self, q: &Request<'_, '_>, out: &mut Out) {
        let object = q.ctx.tp.object_type();
        self.add_instance_members(q, out, object, &[], true, true, false, false, false);
    }

    /// Dart `_addMembersOfEnclosingInstance`.
    fn add_members_of_enclosing_instance(&mut self, q: &Request<'_, '_>, out: &mut Out, element: ElementId) {
        let ctx = q.ctx;
        let Some(instance) = element.cast::<dartr_element::InstanceElement>() else {
            return;
        };
        let referencing = referencing_interface_for(ctx, element);
        let data = ctx.instance(instance);
        let must_be_static = self.cfg.must_be_static;
        for &g in &data.getters.clone() {
            let g = g.raw();
            let is_enum_values = ctx.element_name(g) == Some("values")
                && elem::is_static(ctx, g)
                && elem::is_origin_variable(ctx, g)
                && elem::enclosing(ctx, g).is_some_and(|e| e.tag() == Tag::Enum);
            if (elem::is_origin_declaration(ctx, g) || is_enum_values)
                && (!must_be_static || elem::is_static(ctx, g))
            {
                self.suggest_property(q, out, ElemRef::Base(g), false, None, referencing, true, false, false);
            }
        }
        for &s in &data.setters.clone() {
            let s = s.raw();
            if elem::is_origin_declaration(ctx, s) && (!must_be_static || elem::is_static(ctx, s)) {
                self.suggest_property(q, out, ElemRef::Base(s), false, None, referencing, true, false, false);
            }
        }
        for &f in &data.fields.clone() {
            let f = f.raw();
            if (elem::is_origin_declaration(ctx, f) || elem::is_origin_declaring_formal_parameter(ctx, f))
                && (!must_be_static || elem::is_static(ctx, f))
            {
                self.suggest_field(q, out, ElemRef::Base(f), referencing, true, false, false);
            }
        }
        for &m in &data.methods.clone() {
            let m = m.raw();
            if !must_be_static || elem::is_static(ctx, m) {
                self.suggest_method(q, out, ElemRef::Base(m), false, None, referencing, false, false);
            }
        }
        let this_type = instance_this_type(ctx, element);
        if let Some(this_type) = this_type {
            self.add_extension_members(q, out, this_type, &[], true, true, false, false);
            match ctx.ty(this_type) {
                TypeKind::Record { .. } => {
                    self.add_fields_of_record_type(q, out, this_type, &[], false, false)
                }
                TypeKind::Function(_) => self.suggest_function_call(q, out, this_type, None),
                _ => {}
            }
        }
    }

    /// Dart `_addMembersOfEnclosingNode`.
    fn add_members_of_enclosing_node(&mut self, q: &Request<'_, '_>, out: &mut Out, declaration: NodeId) {
        let ast = q.ast;
        let ctx = q.ctx;
        let Some(element) = q.declared_element(declaration) else {
            return;
        };
        let type_params = |e: ElementId| -> Vec<ElementId> {
            if let Some(i) = e.cast::<dartr_element::InstanceElement>() {
                ctx.instance(i).type_params.iter().map(|p| p.raw()).collect()
            } else if let Some(a) = e.cast::<dartr_element::TypeAliasElement>() {
                ctx.get(a).type_params.iter().map(|p| p.raw()).collect()
            } else {
                Vec::new()
            }
        };
        match ast.kind(declaration) {
            NodeKind::ClassDeclaration
            | NodeKind::EnumDeclaration
            | NodeKind::ExtensionDeclaration
            | NodeKind::MixinDeclaration => {
                if !self.cfg.must_be_type {
                    self.add_members_of_enclosing_instance(q, out, element);
                }
                self.suggest_type_parameters(q, out, &type_params(element));
            }
            NodeKind::ExtensionTypeDeclaration => {
                if !self.cfg.must_be_type {
                    self.add_members_of_enclosing_instance(q, out, element);
                    if let Some(f) = extension_type_representation(ctx, element) {
                        self.suggest_field(q, out, ElemRef::Base(f), None, false, false, false);
                    }
                }
                self.suggest_type_parameters(q, out, &type_params(element));
            }
            NodeKind::ClassTypeAlias | NodeKind::FunctionTypeAlias | NodeKind::GenericTypeAlias => {
                if element.tag() == Tag::TypeAlias || element.tag() == Tag::Class {
                    self.suggest_type_parameters(q, out, &type_params(element));
                }
            }
            _ => {}
        }
    }

    /// Dart `_addStaticMembers`.
    #[allow(clippy::too_many_arguments)]
    fn add_static_members(
        &mut self,
        q: &Request<'_, '_>,
        out: &mut Out,
        getters: &[ElementId],
        setters: &[ElementId],
        constructors: &[ElementId],
        containing: ElementId,
        fields: &[ElementId],
        methods: &[ElementId],
        only_invocations: bool,
    ) {
        let ctx = q.ctx;
        let library = q.library;
        let is_function_like = |t: TypeId| {
            matches!(ctx.ty(t), TypeKind::Function(_)) || ctx.is_dart_core_function(t)
        };
        for &g in getters {
            if elem::is_static(ctx, g)
                && elem::is_origin_declaration(ctx, g)
                && elem::is_visible_in(ctx, g, library)
                && (!only_invocations || is_function_like(member::return_type(ctx, ElemRef::Base(g))))
            {
                self.suggest_property(q, out, ElemRef::Base(g), false, None, None, false, false, false);
            }
        }
        for &s in setters {
            if elem::is_static(ctx, s) && elem::is_origin_declaration(ctx, s) && elem::is_visible_in(ctx, s, library)
            {
                self.suggest_property(q, out, ElemRef::Base(s), false, None, None, false, false, false);
            }
        }
        for &f in fields {
            if elem::is_static(ctx, f)
                && (elem::is_origin_declaration(ctx, f) || elem::is_origin_enum_values(ctx, f))
                && elem::is_visible_in(ctx, f, library)
                && (!only_invocations || is_function_like(member::type_(ctx, ElemRef::Base(f))))
            {
                if is_enum_constant(ctx, f) {
                    let enum_name = elem::enclosing(ctx, f)
                        .and_then(|e| ctx.element_name(e))
                        .unwrap_or("");
                    let score = out.score(&format!("{enum_name}.{}", ctx.element_name(f).unwrap_or("")));
                    if score != -1.0 {
                        out.add(Candidate::new(
                            Kind::EnumConstant {
                                element: f,
                                include_enum_name: false,
                            },
                            score,
                        ));
                    }
                } else {
                    self.suggest_field(q, out, ElemRef::Base(f), None, false, false, false);
                }
            }
        }
        if !self.cfg.must_be_assignable {
            let allow_non_factory = (containing.tag() == Tag::Class && !elem::is_abstract_class(ctx, containing))
                || containing.tag() == Tag::ExtensionType;
            for &c in constructors {
                if elem::is_visible_in(ctx, c, library) && (allow_non_factory || elem::is_factory(ctx, c)) {
                    self.suggest_constructor(q, out, ElemRef::Base(c), None, true, false, None);
                }
            }
            for &m in methods {
                if elem::is_static(ctx, m) && elem::is_visible_in(ctx, m, library) {
                    self.suggest_method(q, out, ElemRef::Base(m), false, None, None, false, false);
                }
            }
        }
    }

    /// Dart `_addTopLevelDeclarations`.
    fn add_top_level_declarations(&mut self, q: &Request<'_, '_>, out: &mut Out) {
        let ctx = q.ctx;
        let l = ctx.get(q.library);
        for &e in &l.classes {
            self.suggest_class(q, out, e.raw(), None);
        }
        for &e in &l.enums {
            self.suggest_enum(q, out, e.raw(), None);
        }
        for &e in &l.extension_types {
            self.suggest_extension_type(q, out, e.raw(), None);
        }
        for &e in &l.mixins {
            self.suggest_mixin(q, out, e.raw(), None);
        }
        for &e in &l.type_aliases {
            self.suggest_type_alias(q, out, e.raw(), None);
        }
        if !self.cfg.must_be_type {
            for &e in &l.getters {
                if elem::is_origin_declaration(ctx, e.raw()) {
                    self.suggest_top_level_property(q, out, e.raw(), None);
                }
            }
            for &e in &l.setters {
                let e = e.raw();
                if elem::is_origin_declaration(ctx, e) && elem::corresponding_getter(ctx, e).is_none() {
                    self.suggest_top_level_property(q, out, e, None);
                }
            }
            for &e in &l.extensions {
                if ctx.element_name(e.raw()).is_some() {
                    self.suggest_extension(q, out, e.raw(), None);
                }
            }
            for &e in &l.top_level_functions {
                self.suggest_top_level_function(q, out, e.raw(), None);
            }
            for &e in &l.top_level_variables {
                if elem::is_origin_declaration(ctx, e.raw()) {
                    self.suggest_top_level_variable(q, out, e.raw(), None);
                }
            }
        }
    }

    /// Dart `_bestMember`.
    fn best_member(&self, ctx: &Ctx<'_>, list: &[ElemRef]) -> ElemRef {
        let first = list[0];
        if self.cfg.must_be_assignable {
            for &m in list {
                let base = member::base_element(ctx, m);
                if base.tag() == Tag::Setter {
                    if elem::is_origin_variable(ctx, base) {
                        if let Some(g) = member::corresponding_getter(ctx, m) {
                            return g;
                        }
                    }
                    return m;
                }
            }
        }
        if member::base_element(ctx, first).tag() == Tag::Setter {
            for &m in &list[1..] {
                if member::base_element(ctx, m).tag() == Tag::Getter {
                    return m;
                }
            }
        }
        first
    }

    /// Dart `_canAccessInstanceMember`.
    fn can_access_instance_member(&self, q: &Request<'_, '_>, m: ElemRef) -> bool {
        let ctx = q.ctx;
        let base = member::base_element(ctx, m);
        if elem::is_static(ctx, base) {
            return false;
        }
        if !elem::is_accessible_in(ctx, base, q.library) {
            return false;
        }
        let library = elem::library_of(ctx, base);
        if dartr_resolver::element_metadata::is_internal(ctx, base, None) {
            if let Some(root) = &q.package_root {
                let path = library.map(|l| q.library_path(l)).unwrap_or_default();
                if !path.starts_with(root.as_str()) {
                    return false;
                }
            }
        }
        if dartr_resolver::element_metadata::is_protected(ctx, base, None) {
            let Some(interface) = elem::enclosing(ctx, base).and_then(|e| e.cast::<InterfaceElement>()) else {
                return false;
            };
            if library != Some(q.library) {
                let Some(context) = q.enclosing_interface_element() else {
                    return false;
                };
                let context_type = ctx.interface_this_type(context);
                if ctx.as_instance_of(context_type, interface).is_none() {
                    return false;
                }
            }
        }
        if dartr_resolver::element_metadata::is_visible_for_testing(ctx, base, None) && library != Some(q.library) {
            if let Some(root) = &q.package_root {
                let path = library.map(|l| q.library_path(l)).unwrap_or_default();
                if !path.starts_with(root.as_str()) {
                    return false;
                }
                if !q.in_test_directory {
                    return false;
                }
            }
        }
        true
    }

    /// Dart `_createSuggestionFromTopLevelProperty`.
    fn top_level_property_kind(&self, ctx: &Ctx<'_>, element: ElementId) -> Option<Kind> {
        if elem::is_origin_variable(ctx, element) {
            if element.tag() == Tag::Getter {
                let v = elem::accessor_variable(ctx, element)?;
                if v.tag() == Tag::TopLevelVariable {
                    return Some(Kind::TopLevelVariable(v));
                }
            }
            None
        } else if element.tag() == Tag::Getter {
            Some(Kind::TopLevelGetter(element))
        } else {
            Some(Kind::TopLevelSetter(element))
        }
    }

    /// Dart `_matchesContextType`.
    fn matches_context_type(&self, q: &Request<'_, '_>, element: ElemRef) -> bool {
        let ctx = q.ctx;
        match q.context_type {
            Some(c) => c == member::type_(ctx, element) || ctx.is_dart_core_function(c),
            None => false,
        }
    }

    fn suggest_class(&mut self, q: &Request<'_, '_>, out: &mut Out, element: ElementId, import: Option<&ImportData>) {
        let ctx = q.ctx;
        if !self.visible(q, element, import) {
            return;
        }
        let other_library = elem::library_of(ctx, element) != Some(q.library);
        let flags = class_flags(ctx, element);
        if self.cfg.must_be_extendable && other_library && !flags.extendable_outside {
            return;
        }
        if self.cfg.must_be_implementable && other_library && !flags.implementable_outside {
            return;
        }
        if self.cfg.must_be_mixable && other_library && !flags.mixable_outside {
            return;
        }
        if !(self.cfg.must_be_constant && !self.cfg.object_pattern_allowed) && !self.cfg.exclude_type_names {
            let score = out.score(&display_name(ctx, element));
            if score != -1.0 {
                out.add(Candidate::with_import(Kind::Class(element), import.cloned(), score));
            }
        }
        if !self.cfg.must_be_type {
            let interface = element.cast::<InterfaceElement>().unwrap();
            let fields: Vec<ElementId> = ctx.instance(interface.raw().cast().unwrap()).fields.iter().map(|f| f.raw()).collect();
            self.suggest_static_fields(q, out, &fields, import);
            let constructors: Vec<ElementId> = ctx.interface(interface).constructors.iter().map(|c| c.raw()).collect();
            self.suggest_constructors(q, out, &constructors, import, !elem::is_abstract_class(ctx, element), None);
        }
    }

    /// Dart `_suggestConstructor`.
    #[allow(clippy::too_many_arguments)]
    fn suggest_constructor(
        &mut self,
        q: &Request<'_, '_>,
        out: &mut Out,
        element: ElemRef,
        import: Option<&ImportData>,
        has_class_name: bool,
        is_redirect: bool,
        alias: Option<ElementId>,
    ) {
        let ctx = q.ctx;
        if self.cfg.must_be_assignable {
            return;
        }
        let base = member::base_element(ctx, element);
        if self.cfg.must_be_constant && !elem::is_const_constructor(ctx, base) {
            return;
        }
        if !elem::is_visible_in(ctx, base, q.library) {
            return;
        }
        if let Some(a) = alias {
            if !elem::is_visible_in(ctx, a, q.library) {
                return;
            }
        }
        let class = elem::enclosing(ctx, base);
        if let Some(e) = alias.or(class) {
            self.visible(q, e, import);
        }
        let element_name = ctx.element_name(base);
        let matcher_name = if self.cfg.suggesting_dot_shorthand && element_name.is_some() {
            element_name.unwrap().to_string()
        } else {
            display_name(ctx, base)
        };
        let score = out.score(&matcher_name);
        if score == -1.0 {
            return;
        }
        let typed = self.typed(q);
        if self.matches_context_type(q, element) && !self.cfg.prefer_non_invocation {
            out.add(Candidate::with_import(
                Kind::Constructor {
                    element,
                    alias,
                    has_class_name,
                    is_tear_off: true,
                    is_redirect,
                    suggest_unnamed_as_new: false,
                    kind: SuggestionKind::Identifier,
                    typed: typed.clone(),
                },
                import.cloned(),
                score,
            ));
        }
        let is_tear_off = self.cfg.prefer_non_invocation;
        out.add(Candidate::with_import(
            Kind::Constructor {
                element,
                alias,
                has_class_name,
                is_tear_off,
                is_redirect,
                suggest_unnamed_as_new: self.cfg.suggest_unnamed_as_new || self.cfg.prefer_non_invocation,
                kind: if is_tear_off || is_redirect {
                    SuggestionKind::Identifier
                } else {
                    SuggestionKind::Invocation
                },
                typed,
            },
            import.cloned(),
            score,
        ));
    }

    /// Dart `_suggestConstructors`.
    fn suggest_constructors(
        &mut self,
        q: &Request<'_, '_>,
        out: &mut Out,
        constructors: &[ElementId],
        import: Option<&ImportData>,
        allow_non_factory: bool,
        alias: Option<ElementId>,
    ) {
        let ctx = q.ctx;
        if self.cfg.must_be_assignable {
            return;
        }
        for &c in constructors {
            if elem::is_visible_in(ctx, c, q.library)
                && alias.is_none_or(|a| elem::is_visible_in(ctx, a, q.library))
                && (allow_non_factory || elem::is_factory(ctx, c))
            {
                self.suggest_constructor(q, out, ElemRef::Base(c), import, false, false, alias);
            }
        }
    }

    fn suggest_enum(&mut self, q: &Request<'_, '_>, out: &mut Out, element: ElementId, import: Option<&ImportData>) {
        let ctx = q.ctx;
        if !self.visible(q, element, import) {
            return;
        }
        if self.cfg.must_be_extendable || self.cfg.must_be_implementable || self.cfg.must_be_mixable {
            return;
        }
        let score = out.score(&display_name(ctx, element));
        if score != -1.0 {
            out.add(Candidate::with_import(Kind::Enum(element), import.cloned(), score));
        }
        if !self.cfg.must_be_type {
            let fields: Vec<ElementId> = ctx.instance(element.cast().unwrap()).fields.iter().map(|f| f.raw()).collect();
            self.suggest_static_fields(q, out, &fields, import);
            let constructors: Vec<ElementId> = ctx
                .interface(element.cast().unwrap())
                .constructors
                .iter()
                .map(|c| c.raw())
                .collect();
            self.suggest_constructors(q, out, &constructors, import, false, None);
        }
    }

    fn suggest_extension(&mut self, q: &Request<'_, '_>, out: &mut Out, element: ElementId, import: Option<&ImportData>) {
        let ctx = q.ctx;
        if !self.visible(q, element, import) {
            return;
        }
        if self.cfg.must_be_extendable || self.cfg.must_be_implementable || self.cfg.must_be_mixable {
            return;
        }
        let score = out.score(&display_name(ctx, element));
        if score != -1.0 {
            out.add(Candidate::with_import(
                Kind::Extension {
                    element,
                    kind: self.executable_kind(),
                },
                import.cloned(),
                score,
            ));
        }
        if !self.cfg.must_be_type {
            let fields: Vec<ElementId> = ctx.instance(element.cast().unwrap()).fields.iter().map(|f| f.raw()).collect();
            self.suggest_static_fields(q, out, &fields, import);
        }
    }

    fn suggest_extension_type(&mut self, q: &Request<'_, '_>, out: &mut Out, element: ElementId, import: Option<&ImportData>) {
        let ctx = q.ctx;
        if !self.visible(q, element, import) {
            return;
        }
        if self.cfg.must_be_extendable || self.cfg.must_be_implementable || self.cfg.must_be_mixable {
            return;
        }
        let score = out.score(&display_name(ctx, element));
        if score != -1.0 {
            out.add(Candidate::with_import(Kind::ExtensionType(element), import.cloned(), score));
        }
        if !self.cfg.must_be_type {
            let fields: Vec<ElementId> = ctx.instance(element.cast().unwrap()).fields.iter().map(|f| f.raw()).collect();
            self.suggest_static_fields(q, out, &fields, import);
            let constructors: Vec<ElementId> = ctx
                .interface(element.cast().unwrap())
                .constructors
                .iter()
                .map(|c| c.raw())
                .collect();
            self.suggest_constructors(q, out, &constructors, import, true, None);
        }
    }

    /// Dart `_suggestField`.
    #[allow(clippy::too_many_arguments)]
    fn suggest_field(
        &mut self,
        q: &Request<'_, '_>,
        out: &mut Out,
        field: ElemRef,
        referencing: Option<ElementId>,
        is_in_declaration: bool,
        _is_keyword_needed: bool,
        _is_type_needed: bool,
    ) {
        let ctx = q.ctx;
        let base = member::base_element(ctx, field);
        if !self.visible(q, base, None) {
            return;
        }
        if (self.cfg.must_be_assignable && elem::variable_setter(ctx, base).is_none())
            || (self.cfg.must_be_constant && !elem::is_const_variable(ctx, base))
        {
            return;
        }
        let score = out.score(&display_name(ctx, base));
        if score != -1.0 {
            out.add(Candidate::new(
                Kind::Field {
                    element: field,
                    referencing_interface: referencing,
                    is_in_declaration,
                    typed: self.typed(q),
                },
                score,
            ));
        }
    }

    /// Dart `_suggestFunctionCall`.
    fn suggest_function_call(&mut self, q: &Request<'_, '_>, out: &mut Out, ty: TypeId, element: Option<ElemRef>) {
        let score = out.score("call");
        if score != -1.0 {
            out.add(Candidate::new(
                Kind::FunctionCall {
                    ty,
                    element,
                    kind: self.executable_kind(),
                    typed: Typed {
                        replacement: q.replacement,
                        ..Typed::default()
                    },
                },
                score,
            ));
        }
    }

    /// Dart `_suggestLocalFunction`.
    fn suggest_local_function(&mut self, q: &Request<'_, '_>, out: &mut Out, element: ElementId) {
        let ctx = q.ctx;
        if element.tag() != Tag::LocalFunction || !self.visible(q, element, None) {
            return;
        }
        let returns_void = matches!(ctx.ty(member::return_type(ctx, ElemRef::Base(element))), TypeKind::Void);
        if self.cfg.must_be_assignable || self.cfg.must_be_constant || (self.cfg.must_be_non_void && returns_void) {
            return;
        }
        if is_wildcard(ctx.element_name(element)) {
            return;
        }
        let score = out.score(&display_name(ctx, element));
        if score == -1.0 {
            return;
        }
        if self.matches_context_type(q, ElemRef::Base(element)) && !self.cfg.prefer_non_invocation {
            out.add(Candidate::new(
                Kind::LocalFunction {
                    element,
                    kind: SuggestionKind::Identifier,
                },
                score,
            ));
        }
        out.add(Candidate::new(
            Kind::LocalFunction {
                element,
                kind: self.executable_kind(),
            },
            score,
        ));
    }

    /// Dart `_suggestMethod`.
    #[allow(clippy::too_many_arguments)]
    fn suggest_method(
        &mut self,
        q: &Request<'_, '_>,
        out: &mut Out,
        method: ElemRef,
        ignore_visibility: bool,
        import: Option<&ImportData>,
        referencing: Option<ElementId>,
        is_keyword_needed: bool,
        is_type_needed: bool,
    ) {
        let ctx = q.ctx;
        let base = member::base_element(ctx, method);
        if !(ignore_visibility || self.visible(q, base, import)) {
            return;
        }
        let is_static = elem::is_static(ctx, base);
        let returns_void = matches!(ctx.ty(member::return_type(ctx, method)), TypeKind::Void);
        if self.cfg.must_be_assignable
            || (self.cfg.must_be_constant && !is_static)
            || (self.cfg.must_be_non_void && returns_void)
        {
            return;
        }
        let score = out.score(&display_name(ctx, base));
        if score == -1.0 {
            return;
        }
        let mut typed = self.typed(q);
        typed.add_type_annotation = is_type_needed && q.style.specify_types;
        if is_keyword_needed {
            if q.style.make_locals_final {
                typed.keyword = Some("final");
            } else if !q.style.specify_types {
                typed.keyword = Some("var");
            }
        }
        let enclosing = elem::enclosing(ctx, base);
        if ctx.element_name(base) == Some("setState")
            && enclosing.is_some_and(|e| e.tag() == Tag::Class && is_exact_state(ctx, e))
        {
            if self.matches_context_type(q, method) && !self.cfg.prefer_non_invocation {
                out.add(Candidate::with_import(
                    Kind::SetState {
                        element: method,
                        referencing_interface: referencing,
                        indent: q.indent(),
                        end_of_line: q.end_of_line(),
                        kind: SuggestionKind::Identifier,
                        typed: Typed {
                            add_type_name: false,
                            ..typed.clone()
                        },
                    },
                    import.cloned(),
                    score,
                ));
            }
            out.add(Candidate::with_import(
                Kind::SetState {
                    element: method,
                    referencing_interface: referencing,
                    indent: q.indent(),
                    end_of_line: q.end_of_line(),
                    kind: SuggestionKind::Invocation,
                    typed: Typed {
                        add_type_name: false,
                        ..typed
                    },
                },
                import.cloned(),
                score,
            ));
            return;
        }
        if self.matches_context_type(q, method) && !self.cfg.prefer_non_invocation {
            out.add(Candidate::with_import(
                Kind::Method {
                    element: method,
                    kind: SuggestionKind::Identifier,
                    referencing_interface: referencing,
                    typed: typed.clone(),
                },
                import.cloned(),
                score,
            ));
        }
        if self.cfg.must_be_constant && (is_static || !self.cfg.prefer_non_invocation) {
            return;
        }
        out.add(Candidate::with_import(
            Kind::Method {
                element: method,
                kind: self.executable_kind(),
                referencing_interface: referencing,
                typed,
            },
            import.cloned(),
            score,
        ));
    }

    fn suggest_mixin(&mut self, q: &Request<'_, '_>, out: &mut Out, element: ElementId, import: Option<&ImportData>) {
        let ctx = q.ctx;
        if !self.visible(q, element, import) {
            return;
        }
        if self.cfg.must_be_extendable {
            return;
        }
        if self.cfg.must_be_implementable
            && elem::library_of(ctx, element) != Some(q.library)
            && !class_flags(ctx, element).implementable_outside
        {
            return;
        }
        let score = out.score(&display_name(ctx, element));
        if score != -1.0 {
            out.add(Candidate::with_import(Kind::Mixin(element), import.cloned(), score));
        }
        if !self.cfg.must_be_type {
            let fields: Vec<ElementId> = ctx.instance(element.cast().unwrap()).fields.iter().map(|f| f.raw()).collect();
            self.suggest_static_fields(q, out, &fields, import);
        }
    }

    fn suggest_parameter(&mut self, q: &Request<'_, '_>, out: &mut Out, element: ElementId) {
        let ctx = q.ctx;
        if !self.visible(q, element, None) {
            return;
        }
        if self.cfg.must_be_constant || is_wildcard(ctx.element_name(element)) {
            return;
        }
        let score = out.score(&display_name(ctx, element));
        if score != -1.0 {
            let distance = self.variable_distance;
            self.variable_distance += 1;
            out.add(Candidate::new(Kind::FormalParameter { element, distance }, score));
        }
    }

    /// Dart `_suggestProperty`.
    #[allow(clippy::too_many_arguments)]
    fn suggest_property(
        &mut self,
        q: &Request<'_, '_>,
        out: &mut Out,
        accessor: ElemRef,
        ignore_visibility: bool,
        import: Option<&ImportData>,
        referencing: Option<ElementId>,
        is_in_declaration: bool,
        is_keyword_needed: bool,
        is_type_needed: bool,
    ) {
        let ctx = q.ctx;
        let base = member::base_element(ctx, accessor);
        if !(ignore_visibility || self.visible(q, base, import)) {
            return;
        }
        let is_getter = base.tag() == Tag::Getter;
        let returns_void = is_getter && matches!(ctx.ty(member::return_type(ctx, accessor)), TypeKind::Void);
        if (self.cfg.must_be_assignable && is_getter && elem::corresponding_setter(ctx, base).is_none())
            || self.cfg.must_be_constant
            || (self.cfg.must_be_non_void && returns_void)
        {
            return;
        }
        let score = out.score(&display_name(ctx, base));
        if score == -1.0 {
            return;
        }
        let mut typed = self.typed(q);
        typed.add_type_annotation = is_type_needed && q.style.specify_types && !is_in_declaration;
        if is_keyword_needed {
            if q.style.make_locals_final {
                typed.keyword = Some("final");
            } else if !q.style.specify_types {
                typed.keyword = Some("var");
            }
        }
        if elem::is_origin_variable(ctx, base) {
            if is_getter {
                if let Some(variable) = member::variable(ctx, accessor) {
                    if member::base_element(ctx, variable).tag() == Tag::Field {
                        out.add(Candidate::new(
                            Kind::Field {
                                element: variable,
                                referencing_interface: referencing,
                                is_in_declaration,
                                typed,
                            },
                            score,
                        ));
                    }
                }
            }
        } else if is_getter {
            out.add(Candidate::with_import(
                Kind::Getter {
                    element: accessor,
                    referencing_interface: referencing,
                    with_enclosing_name: false,
                    typed,
                },
                import.cloned(),
                score,
            ));
        } else {
            out.add(Candidate::with_import(
                Kind::Setter {
                    element: accessor,
                    referencing_interface: referencing,
                    with_enclosing_name: false,
                },
                import.cloned(),
                score,
            ));
        }
    }

    /// Dart `_suggestRecordField`.
    fn suggest_record_field(
        &mut self,
        q: &Request<'_, '_>,
        out: &mut Out,
        field_type: TypeId,
        name: String,
        is_keyword_needed: bool,
        is_type_needed: bool,
    ) {
        let score = out.score(&name);
        if score == -1.0 {
            return;
        }
        let mut keyword = None;
        if is_keyword_needed {
            if q.style.make_locals_final {
                keyword = Some("final");
            } else if !q.style.specify_types {
                keyword = Some("var");
            }
        }
        out.add(Candidate::new(
            Kind::RecordField {
                field_type,
                name,
                typed: Typed {
                    add_type_annotation: is_type_needed && q.style.specify_types,
                    replacement: q.replacement,
                    keyword,
                    ..Typed::default()
                },
            },
            score,
        ));
    }

    /// Dart `_suggestStaticField`.
    fn suggest_static_field(&mut self, q: &Request<'_, '_>, out: &mut Out, element: ElementId, import: Option<&ImportData>) {
        let ctx = q.ctx;
        let enclosing_is_enum = elem::enclosing(ctx, element).is_some_and(|e| e.tag() == Tag::Enum);
        let enum_constant = is_enum_constant(ctx, element);
        if !elem::is_static(ctx, element)
            || (self.cfg.must_be_assignable
                && !(elem::is_final_variable(ctx, element) || elem::is_const_variable(ctx, element)))
            || (self.cfg.must_be_constant && !elem::is_const_variable(ctx, element))
            || (!enum_constant && enclosing_is_enum)
        {
            return;
        }
        let Some(context) = q.context_type else {
            return;
        };
        let ty = member::type_(ctx, ElemRef::Base(element));
        if !TypeSystem::new(*ctx).is_subtype_of(ty, context) {
            return;
        }
        if enum_constant {
            let enum_name = elem::enclosing(ctx, element)
                .map(|e| display_name(ctx, e))
                .unwrap_or_default();
            let score = out.score(&format!("{enum_name}.{}", display_name(ctx, element)));
            if score != -1.0 {
                out.add(Candidate::with_import(
                    Kind::EnumConstant {
                        element,
                        include_enum_name: true,
                    },
                    import.cloned(),
                    score,
                ));
            }
            return;
        }
        let score = out.score(&display_name(ctx, element));
        if score == -1.0 {
            return;
        }
        if elem::is_origin_getter_setter(ctx, element) {
            if let Some(getter) = elem::variable_getter(ctx, element) {
                if elem::is_origin_variable(ctx, getter) {
                    if let Some(v) = elem::accessor_variable(ctx, getter).filter(|v| v.tag() == Tag::Field) {
                        out.add(Candidate::new(
                            Kind::Field {
                                element: ElemRef::Base(v),
                                referencing_interface: None,
                                is_in_declaration: false,
                                typed: self.typed(q),
                            },
                            score,
                        ));
                    }
                } else {
                    out.add(Candidate::with_import(
                        Kind::Getter {
                            element: ElemRef::Base(getter),
                            referencing_interface: None,
                            with_enclosing_name: true,
                            typed: self.typed(q),
                        },
                        import.cloned(),
                        score,
                    ));
                }
            }
        } else {
            out.add(Candidate::with_import(Kind::StaticField(element), import.cloned(), score));
        }
    }

    fn suggest_static_fields(&mut self, q: &Request<'_, '_>, out: &mut Out, fields: &[ElementId], import: Option<&ImportData>) {
        for &f in fields {
            if elem::is_visible_in(q.ctx, f, q.library) {
                self.suggest_static_field(q, out, f, import);
            }
        }
    }

    fn suggest_super_parameter(&mut self, out: &mut Out, ctx: &Ctx<'_>, element: ElemRef) {
        let score = out.score(&display_name(ctx, member::base_element(ctx, element)));
        if score != -1.0 {
            out.add(Candidate::new(Kind::SuperParameter(element), score));
        }
    }

    fn suggest_top_level_function(&mut self, q: &Request<'_, '_>, out: &mut Out, element: ElementId, import: Option<&ImportData>) {
        let ctx = q.ctx;
        if !self.visible(q, element, import) {
            return;
        }
        let returns_void = matches!(ctx.ty(member::return_type(ctx, ElemRef::Base(element))), TypeKind::Void);
        if self.cfg.must_be_assignable || (self.cfg.must_be_non_void && returns_void) || self.cfg.must_be_type {
            return;
        }
        let score = out.score(&display_name(ctx, element));
        if score == -1.0 {
            return;
        }
        if self.matches_context_type(q, ElemRef::Base(element)) && !self.cfg.prefer_non_invocation {
            out.add(Candidate::with_import(
                Kind::TopLevelFunction {
                    element,
                    kind: SuggestionKind::Identifier,
                },
                import.cloned(),
                score,
            ));
        }
        if !self.cfg.prefer_non_invocation && self.cfg.must_be_constant {
            return;
        }
        out.add(Candidate::with_import(
            Kind::TopLevelFunction {
                element,
                kind: self.executable_kind(),
            },
            import.cloned(),
            score,
        ));
    }

    fn suggest_top_level_property(&mut self, q: &Request<'_, '_>, out: &mut Out, element: ElementId, import: Option<&ImportData>) {
        let ctx = q.ctx;
        if !self.visible(q, element, import) {
            return;
        }
        let is_getter = element.tag() == Tag::Getter;
        let returns_void = is_getter
            && matches!(ctx.ty(member::return_type(ctx, ElemRef::Base(element))), TypeKind::Void);
        if (self.cfg.must_be_assignable && is_getter && elem::corresponding_setter(ctx, element).is_none())
            || (self.cfg.must_be_constant && !elem::accessor_is_const(ctx, element))
            || (self.cfg.must_be_non_void && returns_void)
            || self.cfg.must_be_type
        {
            return;
        }
        let score = out.score(&display_name(ctx, element));
        if score != -1.0 {
            if let Some(kind) = self.top_level_property_kind(ctx, element) {
                out.add(Candidate::with_import(kind, import.cloned(), score));
            }
        }
    }

    fn suggest_top_level_variable(&mut self, q: &Request<'_, '_>, out: &mut Out, element: ElementId, import: Option<&ImportData>) {
        let ctx = q.ctx;
        if !self.visible(q, element, import) {
            return;
        }
        if (self.cfg.must_be_assignable && elem::variable_setter(ctx, element).is_none())
            || (self.cfg.must_be_constant && !elem::is_const_variable(ctx, element))
            || self.cfg.must_be_type
        {
            return;
        }
        let score = out.score(&display_name(ctx, element));
        if score != -1.0 {
            out.add(Candidate::with_import(Kind::TopLevelVariable(element), import.cloned(), score));
        }
    }

    fn suggest_type_alias(&mut self, q: &Request<'_, '_>, out: &mut Out, element: ElementId, import: Option<&ImportData>) {
        let ctx = q.ctx;
        if !self.visible(q, element, import) {
            return;
        }
        let score = out.score(&display_name(ctx, element));
        if score != -1.0 {
            out.add(Candidate::with_import(Kind::TypeAlias(element), import.cloned(), score));
        }
        if !self.cfg.must_be_type {
            self.add_constructors_for_aliased_element(q, out, element, import);
        }
    }

    fn suggest_type_parameter(&mut self, q: &Request<'_, '_>, out: &mut Out, element: ElementId) {
        let ctx = q.ctx;
        if !self.visible(q, element, None) {
            return;
        }
        let score = out.score(&display_name(ctx, element));
        if score != -1.0 {
            out.add(Candidate::new(Kind::TypeParameter(element), score));
        }
    }

    fn suggest_type_parameters(&mut self, q: &Request<'_, '_>, out: &mut Out, params: &[ElementId]) {
        for &p in params {
            if !is_wildcard(q.ctx.element_name(p)) {
                self.suggest_type_parameter(q, out, p);
            }
        }
    }

    fn suggest_variable(&mut self, q: &Request<'_, '_>, out: &mut Out, element: ElementId) {
        let ctx = q.ctx;
        if is_wildcard(ctx.element_name(element)) && q.wildcard_variables() {
            return;
        }
        if !self.visible(q, element, None) {
            return;
        }
        if self.cfg.must_be_constant && !elem::is_const_variable(ctx, element) {
            return;
        }
        let score = out.score(&display_name(ctx, element));
        if score != -1.0 {
            let distance = self.variable_distance;
            self.variable_distance += 1;
            out.add(Candidate::new(Kind::LocalVariable { element, distance }, score));
        }
    }

    fn visit_catch_clause(&mut self, q: &Request<'_, '_>, out: &mut Out, node: NodeId) {
        let ast = q.ast;
        let c = ast.cast::<CatchClause>(node).unwrap();
        if let Some(p) = ast[c].exception_parameter {
            if let Some(e) = q.declared_element(p.raw()) {
                self.suggest_variable(q, out, e);
            }
        }
        if let Some(p) = ast[c].stack_trace_parameter {
            if let Some(e) = q.declared_element(p.raw()) {
                self.suggest_variable(q, out, e);
            }
        }
    }

    fn visit_comment_reference(&mut self, q: &Request<'_, '_>, out: &mut Out, node: NodeId) -> Option<NodeId> {
        let ast = q.ast;
        let comment = ast.parent(node);
        let member = comment.and_then(|c| ast.parent(c));
        if let Some(m) = member {
            if let Some(d) = ast.cast::<ConstructorDeclaration>(m) {
                self.visit_parameter_list(q, out, Some(ast[d].parameters));
            } else if let Some(d) = ast.cast::<FunctionDeclaration>(m) {
                let f = ast[d].function_expression;
                self.visit_parameter_list(q, out, ast[f].parameters);
                self.visit_type_parameter_list(q, out, ast[f].type_parameters);
            } else if let Some(f) = ast.cast::<FunctionExpression>(m) {
                self.visit_parameter_list(q, out, ast[f].parameters);
                self.visit_type_parameter_list(q, out, ast[f].type_parameters);
            } else if let Some(d) = ast.cast::<MethodDeclaration>(m) {
                self.visit_parameter_list(q, out, ast[d].parameters);
                self.visit_type_parameter_list(q, out, ast[d].type_parameters);
            }
        }
        comment
    }

    fn visit_declared_variable_pattern(&mut self, q: &Request<'_, '_>, out: &mut Out, node: NodeId) {
        if let Some(e) = q.declared_element(node) {
            self.suggest_variable(q, out, e);
        }
    }

    fn visit_for_loop_parts(&mut self, q: &Request<'_, '_>, out: &mut Out, node: NodeId) {
        let ast = q.ast;
        if let Some(p) = ast.cast::<ForEachPartsWithDeclaration>(node) {
            if let Some(e) = q.declared_element(ast[p].loop_variable.raw()) {
                self.suggest_variable(q, out, e);
            }
        } else if let Some(p) = ast.cast::<ForEachPartsWithPattern>(node) {
            self.visit_pattern(q, out, ast[p].pattern.raw());
        } else if let Some(p) = ast.cast::<ForPartsWithDeclarations>(node) {
            let variables = ast[p].variables;
            for &v in ast.list(ast[variables].variables) {
                if let Some(e) = q.declared_element(v.raw()) {
                    if is_local_variable(e) {
                        self.suggest_variable(q, out, e);
                    }
                }
            }
        } else if let Some(p) = ast.cast::<ForPartsWithPattern>(node) {
            let pattern = ast[ast[p].variables].pattern;
            self.visit_pattern(q, out, pattern.raw());
        }
    }

    fn visit_parameter_list(&mut self, q: &Request<'_, '_>, out: &mut Out, list: Option<Id<FormalParameterList>>) {
        let ast = q.ast;
        let Some(list) = list else {
            return;
        };
        for &p in ast.list(ast[list].parameters) {
            if let Some(e) = q.declared_element(p.raw()) {
                self.suggest_parameter(q, out, e);
            }
        }
    }

    fn visit_pattern(&mut self, q: &Request<'_, '_>, out: &mut Out, pattern: NodeId) {
        let ast = q.ast;
        match ast.kind(pattern) {
            NodeKind::CastPattern => {
                let p = ast.cast::<CastPattern>(pattern).unwrap();
                self.visit_pattern(q, out, ast[p].pattern.raw());
            }
            NodeKind::DeclaredVariablePattern => self.visit_declared_variable_pattern(q, out, pattern),
            NodeKind::ListPattern => {
                let p = ast.cast::<ListPattern>(pattern).unwrap();
                for &e in ast.list_raw(ast[p].elements) {
                    if ast.is::<DartPattern>(e) {
                        self.visit_pattern(q, out, e);
                    } else if let Some(r) = ast.cast::<RestPatternElement>(e) {
                        if let Some(p) = ast[r].pattern {
                            self.visit_pattern(q, out, p.raw());
                        }
                    }
                }
            }
            NodeKind::LogicalAndPattern => {
                let p = ast.cast::<LogicalAndPattern>(pattern).unwrap();
                self.visit_pattern(q, out, ast[p].left_operand.raw());
                self.visit_pattern(q, out, ast[p].right_operand.raw());
            }
            NodeKind::LogicalOrPattern => {
                let p = ast.cast::<LogicalOrPattern>(pattern).unwrap();
                self.visit_pattern(q, out, ast[p].left_operand.raw());
                self.visit_pattern(q, out, ast[p].right_operand.raw());
            }
            NodeKind::MapPattern => {
                let p = ast.cast::<MapPattern>(pattern).unwrap();
                for &e in ast.list_raw(ast[p].elements) {
                    if let Some(entry) = ast.cast::<MapPatternEntry>(e) {
                        self.visit_pattern(q, out, ast[entry].value.raw());
                    } else if let Some(r) = ast.cast::<RestPatternElement>(e) {
                        if let Some(p) = ast[r].pattern {
                            self.visit_pattern(q, out, p.raw());
                        }
                    }
                }
            }
            NodeKind::NullAssertPattern => {
                let p = ast.cast::<NullAssertPattern>(pattern).unwrap();
                self.visit_pattern(q, out, ast[p].pattern.raw());
            }
            NodeKind::NullCheckPattern => {
                let p = ast.cast::<NullCheckPattern>(pattern).unwrap();
                self.visit_pattern(q, out, ast[p].pattern.raw());
            }
            NodeKind::ObjectPattern => {
                let p = ast.cast::<ObjectPattern>(pattern).unwrap();
                for &f in ast.list(ast[p].fields) {
                    self.visit_pattern(q, out, ast[f].pattern.raw());
                }
            }
            NodeKind::ParenthesizedPattern => {
                let p = ast.cast::<ParenthesizedPattern>(pattern).unwrap();
                self.visit_pattern(q, out, ast[p].pattern.raw());
            }
            NodeKind::RecordPattern => {
                let p = ast.cast::<RecordPattern>(pattern).unwrap();
                for &f in ast.list(ast[p].fields) {
                    self.visit_pattern(q, out, ast[f].pattern.raw());
                }
            }
            _ => {}
        }
    }

    /// Dart `_visitStatements`.
    fn visit_statements(&mut self, q: &Request<'_, '_>, out: &mut Out, statements: &[NodeId], child: Option<NodeId>) {
        let ast = q.ast;
        for &statement in statements.iter().rev() {
            if Some(statement) == child {
                continue;
            }
            if ast.offset(statement) >= q.offset {
                continue;
            }
            if let Some(s) = ast.cast::<VariableDeclarationStatement>(statement) {
                let variables = ast[s].variables;
                for &v in ast.list(ast[variables].variables) {
                    if ast.end(v) < q.offset {
                        if let Some(e) = q.declared_element(v.raw()) {
                            if is_local_variable(e) {
                                self.suggest_variable(q, out, e);
                            }
                        }
                    }
                }
            } else if let Some(s) = ast.cast::<FunctionDeclarationStatement>(statement) {
                let d = ast[s].function_declaration;
                if ast.offset(d) < q.offset && !ast.t_lexeme(ast[d].name).is_empty() {
                    if let Some(e) = q.declared_element(d.raw()) {
                        self.suggest_local_function(q, out, e);
                    }
                }
            } else if let Some(s) = ast.cast::<PatternVariableDeclarationStatement>(statement) {
                let d = ast[s].declaration;
                if ast.end(d) < q.offset {
                    self.visit_pattern(q, out, ast[d].pattern.raw());
                }
            }
        }
    }

    fn visit_switch_pattern_case(&mut self, q: &Request<'_, '_>, out: &mut Out, node: NodeId, child: Option<NodeId>) {
        let ast = q.ast;
        let c = ast.cast::<SwitchPatternCase>(node).unwrap();
        if q.offset < ast.t_end(ast[c].colon) {
            return;
        }
        let statements: Vec<NodeId> = ast.list_raw(ast[c].statements).to_vec();
        self.visit_statements(q, out, &statements, child);
        let pattern = ast[ast[c].guarded_pattern].pattern;
        self.visit_pattern(q, out, pattern.raw());
        if let Some(s) = ast.parent(node).and_then(|p| ast.cast::<SwitchStatement>(p)) {
            let members = ast.list_raw(ast[s].members);
            if let Some(index) = members.iter().position(|m| *m == node) {
                for &m in members[..index].iter().rev() {
                    match ast.cast::<SwitchPatternCase>(m) {
                        Some(pc) if ast.list(ast[pc].statements).is_empty() => {
                            let pattern = ast[ast[pc].guarded_pattern].pattern;
                            self.visit_pattern(q, out, pattern.raw());
                        }
                        _ => break,
                    }
                }
            }
        }
    }

    fn visit_type_parameter_list(&mut self, q: &Request<'_, '_>, out: &mut Out, list: Option<Id<TypeParameterList>>) {
        let ast = q.ast;
        let Some(list) = list else {
            return;
        };
        if self.cfg.excluded_nodes.contains(&list.raw()) {
            return;
        }
        for &p in ast.list(ast[list].type_parameters) {
            if let Some(e) = q.declared_element(p.raw()) {
                if !is_wildcard(q.ctx.element_name(e)) {
                    self.suggest_type_parameter(q, out, e);
                }
            }
        }
    }
}

fn is_local_variable(e: ElementId) -> bool {
    matches!(
        e.tag(),
        Tag::LocalVariable | Tag::PatternVariable | Tag::BindPatternVariable | Tag::JoinPatternVariable
    )
}

/// Dart `_referencingInterfaceFor`.
fn referencing_interface_for(ctx: &Ctx<'_>, element: ElementId) -> Option<ElementId> {
    if element.is::<InterfaceElement>() {
        return Some(element);
    }
    let this_type = instance_this_type(ctx, element)?;
    ctx.interface_element(this_type).map(|e| e.raw())
}

/// Dart `InstanceElement.thisType`.
pub fn instance_this_type(ctx: &Ctx<'_>, element: ElementId) -> Option<TypeId> {
    if let Some(i) = element.cast::<InterfaceElement>() {
        return Some(ctx.interface_this_type(i));
    }
    if let Some(e) = element.cast::<ExtensionElement>() {
        return ctx.get(e).extended_type.get();
    }
    None
}

/// Dart `ExtensionTypeElement.representation`.
fn extension_type_representation(ctx: &Ctx<'_>, element: ElementId) -> Option<ElementId> {
    let instance = element.cast::<dartr_element::InstanceElement>()?;
    ctx.instance(instance)
        .fields
        .iter()
        .map(|f| f.raw())
        .find(|f| !elem::is_static(ctx, *f))
}

/// Dart `ClassElement.isExactState` (Flutter `State`).
fn is_exact_state(ctx: &Ctx<'_>, class: ElementId) -> bool {
    ctx.element_name(class) == Some("State")
        && elem::library_of(ctx, class)
            .is_some_and(|l| elem::library_uri(ctx, l) == "package:flutter/src/widgets/framework.dart")
}

/// The class modifiers that completion reads.
struct ClassFlags {
    extendable_outside: bool,
    implementable_outside: bool,
    mixable_outside: bool,
}

fn class_flags(ctx: &Ctx<'_>, element: ElementId) -> ClassFlags {
    use dartr_element::FragmentFlags as F;
    let flags = ctx
        .element_data(element)
        .and_then(|d| ctx.fragment_data(d.first_fragment))
        .map(|f| f.flags.get())
        .unwrap_or_default();
    if element.tag() == Tag::Mixin {
        return ClassFlags {
            extendable_outside: false,
            implementable_outside: !flags.contains(F::MIXIN_FRAGMENT_IS_BASE),
            mixable_outside: true,
        };
    }
    let is_interface = flags.contains(F::CLASS_FRAGMENT_IS_INTERFACE);
    let is_final = flags.contains(F::CLASS_FRAGMENT_IS_FINAL);
    let is_sealed = flags.contains(F::CLASS_FRAGMENT_IS_SEALED);
    let is_base = flags.contains(F::CLASS_FRAGMENT_IS_BASE);
    let is_mixin_class = flags.contains(F::CLASS_FRAGMENT_IS_MIXIN_CLASS);
    ClassFlags {
        extendable_outside: !is_interface && !is_final && !is_sealed,
        implementable_outside: !is_base && !is_final && !is_sealed,
        mixable_outside: is_mixin_class && !is_interface && !is_final && !is_sealed,
    }
}

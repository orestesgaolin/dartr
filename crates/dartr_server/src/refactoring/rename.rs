// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_rename.dart (PrepareRenameHandler, RenameHandler)
// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/refactoring.dart (RenameRefactoring.create, getElementToRename, RefactoringWorkspace)
// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/rename.dart (RenameProcessor2, RenameRefactoringImpl)
// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/refactoring_internal.dart (getSourceReferences, SourceReference)
// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/rename_local.dart
// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/rename_parameter.dart
// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/rename_label.dart
// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/rename_library.dart
// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/rename_type_parameter.dart
// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/rename_unit_member.dart
// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/rename_class_member.dart
// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/rename_extension_member.dart
// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/rename_constructor.dart
// Dart source: pkg/analysis_server/lib/src/services/refactoring/legacy/rename_import.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/util.dart (getElementKindName, getElementQualifiedName)
// Dart source: pkg/analysis_server/lib/src/services/correction/namespace.dart (getImportElement)
// Dart source: pkg/analysis_server/lib/src/services/search/hierarchy.dart (getChildren, addNamedSuperFormalParameters, getHierarchyPositionalParameters)
// Dart source: pkg/analyzer/lib/src/dart/analysis/search.dart (ImportElementReferencesVisitor, classMembers, _getEnclosingFragment)

//! `textDocument/prepareRename` and `textDocument/rename`: the rename
//! refactorings of Dart's legacy refactoring framework.

use std::collections::HashMap;

use dartr_ast::*;
use dartr_element::{Ctx, ElemRef, ElementId, FragmentFlags, NoopSink, Tag};
use dartr_resolver::error::support;
use dartr_typesystem::{TypeExt, member};

use super::host::RefactoringHost;
use super::naming;
use super::status::RefactoringStatus;
use crate::correction::change::{SourceChange, SourceEdit, SourceFileEdit, utf16_len};
use crate::index::RelationKind;
use crate::server::{Match, ResolvedUnitRef, SElem, children_named};

/// The element to rename: an element, or an import (Dart
/// `MockLibraryImportElement`): the resolved unit with the import and the
/// index of the import among the imports of the unit.
#[derive(Clone)]
pub enum Target {
    Element(SElem),
    Import {
        unit: ResolvedUnitRefClone,
        index: usize,
    },
}

/// A cloneable [ResolvedUnitRef].
#[derive(Clone)]
pub struct ResolvedUnitRefClone {
    pub library: std::sync::Arc<dartr_cli::driver_provider::ResolvedLibraryResult>,
    pub index: usize,
}

impl ResolvedUnitRefClone {
    pub fn of(r: &ResolvedUnitRef) -> Self {
        ResolvedUnitRefClone {
            library: r.library.clone(),
            index: r.index,
        }
    }

    pub fn unit_ref(&self) -> ResolvedUnitRef {
        ResolvedUnitRef {
            library: self.library.clone(),
            index: self.index,
        }
    }

    pub fn path(&self) -> String {
        self.library.inputs[self.index].path.to_string()
    }
}

/// The kind of a rename refactoring (Dart `RenameRefactoring.create`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Import,
    UnitMember,
    Constructor,
    Label,
    Library,
    ClassMember { interface: ElementId },
    Parameter,
    Local,
    TypeParameter,
    ExtensionMember { extension: ElementId },
}

/// A rename refactoring.
pub struct Rename {
    pub kind: Kind,
    pub target: Target,
    pub old_name: String,
    pub new_name: String,
    /// Unit member: the `State` class of a stateful widget.
    flutter_state: Option<SElem>,
    /// Class member: the elements to rename and the references.
    class_elements: Vec<SElem>,
    class_references: Vec<Match>,
    class_unshadowed: Vec<(usize, crate::index::ElementKey)>,
    /// Parameter: the hierarchy parameters.
    parameters: Vec<SElem>,
    rename_all_positional: bool,
    /// Extension member: the references.
    extension_references: Vec<Match>,
}

/// The edits of a change in the order the files are first edited (Dart
/// `ChangeBuilder`).
#[derive(Default)]
pub(crate) struct ChangeAcc {
    files: Vec<SourceFileEdit>,
    next_id: u64,
}

impl ChangeAcc {
    pub(crate) fn add(&mut self, file: &str, offset: u32, length: u32, replacement: &str) {
        let index = match self.files.iter().position(|f| f.file == file) {
            Some(i) => i,
            None => {
                self.files.push(SourceFileEdit::new(file, 0));
                self.files.len() - 1
            }
        };
        self.next_id += 1;
        let edit = SourceEdit {
            offset,
            length,
            replacement: replacement.to_string(),
            id: self.next_id,
            description: None,
        };
        // Dart throws a `ConflictingEditException`; the same edit twice is
        // kept once.
        let _ = self.files[index].add(edit, false);
    }

    pub(crate) fn into_change(self, message: String) -> SourceChange {
        SourceChange {
            message,
            edits: self.files,
            ..Default::default()
        }
    }
}

/// Dart `formatList`.
fn format_list(pattern: &str, args: &[&str]) -> String {
    let mut out = pattern.to_string();
    for (i, a) in args.iter().enumerate() {
        out = out.replace(&format!("{{{i}}}"), a);
    }
    out
}

/// Dart `capitalize`.
fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Dart `element.kind.displayName`.
fn kind_name(e: ElementId) -> &'static str {
    e.kind().display_name()
}

/// Dart `Element.displayName`.
fn display_name(ctx: &Ctx<'_>, e: ElementId) -> String {
    support::display_name(ctx, e)
}

fn enclosing(ctx: &Ctx<'_>, e: ElementId) -> Option<ElementId> {
    ctx.element_data(e).and_then(|d| d.enclosing)
}

fn is_parameter(e: ElementId) -> bool {
    matches!(
        e.tag(),
        Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter
    )
}

fn is_interface(e: ElementId) -> bool {
    e.cast::<dartr_element::InterfaceElement>().is_some()
}

/// Dart `LocalElement`.
fn is_local(e: ElementId) -> bool {
    matches!(
        e.tag(),
        Tag::LocalVariable
            | Tag::LocalFunction
            | Tag::PatternVariable
            | Tag::BindPatternVariable
            | Tag::JoinPatternVariable
    )
}

fn parameter_is_named(ctx: &Ctx<'_>, e: ElementId) -> bool {
    e.cast::<dartr_element::FormalParameterElement>()
        .is_some_and(|p| ctx.get(p).kind.is_named())
}

/// The path of the library fragment of the first fragment of [e].
pub(crate) fn element_path(ctx: &Ctx<'_>, e: ElementId) -> Option<String> {
    let first = ctx.element_data(e)?.first_fragment;
    crate::navigation::fragment_path(ctx, first)
}

/// Dart `element.library?.isInSdk`.
fn is_in_sdk(ctx: &Ctx<'_>, e: ElementId) -> bool {
    support::library_of(ctx, e).is_some_and(|l| ctx.library_uri(l).starts_with("dart:"))
}

/// The name range of each fragment of [e]: (path, offset, length).
fn fragment_name_ranges(
    ctx: &Ctx<'_>,
    e: ElementId,
) -> Vec<(String, u32, u32, dartr_element::FragmentId)> {
    let mut out = Vec::new();
    let mut f = ctx.element_data(e).map(|d| d.first_fragment);
    while let Some(id) = f {
        let Some(data) = ctx.fragment_data(id) else {
            break;
        };
        let path = crate::navigation::fragment_path(ctx, id);
        let mut name_offset = data.name_offset.or(if id.tag() == Tag::Label {
            data.first_token_offset
        } else {
            None
        });
        let mut name_length = data.name.map(|n| utf16_len(ctx.name_str(n)));
        if name_offset.is_none()
            && let Some(c) = id.cast::<dartr_element::ConstructorFragment>()
        {
            let c = ctx.fragment(c);
            name_offset = c.type_name_offset;
            name_length = c.type_name.map(|n| utf16_len(ctx.name_str(n)));
        }
        if let (Some(path), Some(o), Some(l)) = (path, name_offset, name_length) {
            out.push((path, o, l, id));
        }
        f = data.next_fragment;
    }
    out
}

/// The rename refactorings over a [RefactoringHost] (the LSP server or
/// the legacy server).
pub struct Engine<'h> {
    pub host: &'h mut dyn RefactoringHost,
}

impl Engine<'_> {
    pub(crate) fn require_resolved_unit_in(
        &mut self,
        path: &str,
        context: Option<usize>,
    ) -> Result<ResolvedUnitRef, ()> {
        self.host.resolved_unit_in(path, context).ok_or(())
    }

    pub(crate) fn search_references(&mut self, element: &SElem) -> Vec<Match> {
        self.host.search_references(element)
    }

    pub(crate) fn hierarchy_members_and_parameters(
        &mut self,
        member: &SElem,
    ) -> (Vec<SElem>, Vec<SElem>) {
        self.host.hierarchy_members_and_parameters(member)
    }

    fn hierarchy_named_parameters(&mut self, parameter: &SElem) -> Vec<SElem> {
        self.host.hierarchy_named_parameters(parameter)
    }

    fn append_all_subtypes(
        &mut self,
        class: &SElem,
        all: &mut Vec<SElem>,
        keys: &mut Vec<(usize, crate::index::ElementKey)>,
    ) {
        self.host.append_all_subtypes(class, all, keys)
    }

    fn search_scope(&mut self) -> std::sync::Arc<crate::server::SearchScope> {
        self.host.search_scope()
    }

    fn mentions_any(&mut self, path: &str, names: &[String]) -> bool {
        self.host.mentions_any(path, names)
    }
}

impl Engine<'_> {
    // ---------------------------------------------------------------------
    // Workspace.

    /// Dart `RefactoringWorkspace.containsFile`.
    pub fn workspace_contains_file(&self, path: &str) -> bool {
        self.host
            .collection()
            .is_some_and(|c| c.contexts.iter().any(|ctx| ctx.root.is_analyzed(path)))
    }

    /// Dart `RefactoringWorkspace.containsElement`.
    pub(crate) fn workspace_contains(&self, target: &Target) -> bool {
        match target {
            Target::Import { unit, .. } => self.workspace_contains_file(&unit.path()),
            Target::Element(e) => e
                .with(|ctx| element_path(ctx, e.id))
                .is_some_and(|p| self.workspace_contains_file(&p)),
        }
    }

    // ---------------------------------------------------------------------
    // The element to rename.

    /// Dart `nodeCovering` + `getElement(useMockForImport: true)` +
    /// `RenameRefactoring.getElementToRename`: the target, and the offset
    /// (`u32::MAX`: the name does not exist yet) and length of its name.
    /// Dart `RenameRefactoringImpl.elementKindName`.
    pub fn element_kind_name(&self, rename: &Rename) -> String {
        match &rename.target {
            Target::Element(e) => kind_name(e.id).to_string(),
            // `MockLibraryImportElement.kind` is `ElementKind.IMPORT`.
            Target::Import { .. } => "import directive".to_string(),
        }
    }

    pub fn element_to_rename(
        &mut self,
        resolved: &ResolvedUnitRef,
        offset: u32,
    ) -> Option<(Target, u32, u32)> {
        element_to_rename_in_unit(resolved, offset)
    }

    /// Dart `RenameRefactoring.create`.
    pub fn create_rename(&mut self, resolved: &ResolvedUnitRef, target: Target) -> Option<Rename> {
        create_rename_for_target(resolved, target)
    }
}

pub fn element_to_rename_in_unit(
    resolved: &ResolvedUnitRef,
    offset: u32,
) -> Option<(Target, u32, u32)> {
    let sink = NoopSink;
    let ctx = resolved.ctx(&sink);
    let unit = resolved.unit();
    let ast = &unit.ast;
    let u = crate::element_locator::Unit {
        ctx: &ctx,
        ast,
        tables: &unit.tables,
    };
    let mut node = ast.node_covering(unit.unit, offset, 0)?;
    // `getElement(useMockForImport: true)`.
    let mut import_index: Option<usize> = None;
    let mut element: Option<ElementId> = None;
    if let Some(i) = ast.cast::<ImportDirective>(node) {
        import_index = import_index_of(ast, unit.unit, i);
    } else {
        element = crate::element_locator::get_element(&u, node);
    }
    if let (Some(s), Some(e)) = (ast.cast::<SimpleIdentifier>(node), element)
        && e.tag() == Tag::Prefix
    {
        element = None;
        import_index = prefixed_import_index(&ctx, ast, unit, resolved, s);
        import_index?;
    }
    if element.is_none() && import_index.is_none() {
        return None;
    }
    // `getElementToRename`.
    if let (Some(s), Some(e)) = (ast.cast::<SimpleIdentifier>(node), element)
        && ast.parent(s).is_some_and(|p| ast.is::<Annotation>(p))
        && e.tag() == Tag::Constructor
        && let Some(type_element) = support::element_of(&ctx, &unit.tables, s)
        && type_element
            .cast::<dartr_element::InstanceElement>()
            .is_some()
    {
        element = Some(type_element);
    }
    if ast.is::<ClassNamePart>(node) {
        node = ast.parent(node)?;
    }
    let token_range = |t: dartr_syntax::TokenId| {
        let t = ast.tokens.get(t);
        (t.offset, t.end() - t.offset)
    };
    let node_range = |n: NodeId| (ast.offset(n), ast.length(n));
    let range: Option<(u32, u32)> = match ast.kind(node) {
        NodeKind::AssignedVariablePattern => Some(token_range(
            ast[ast.cast::<AssignedVariablePattern>(node)?].name,
        )),
        NodeKind::ConstructorDeclaration | NodeKind::ConstructorSelector | NodeKind::DottedName => {
            Some(node_range(node))
        }
        NodeKind::DeclaredIdentifier => {
            Some(token_range(ast[ast.cast::<DeclaredIdentifier>(node)?].name))
        }
        NodeKind::DeclaredVariablePattern => Some(token_range(
            ast[ast.cast::<DeclaredVariablePattern>(node)?].name,
        )),
        NodeKind::EnumConstantDeclaration => Some(token_range(
            ast[ast.cast::<EnumConstantDeclaration>(node)?].name,
        )),
        NodeKind::ExtensionDeclaration => {
            let e = ast.cast::<ExtensionDeclaration>(node)?;
            ast[e].name.map(token_range)
        }
        NodeKind::ExtensionOverride => {
            Some(token_range(ast[ast.cast::<ExtensionOverride>(node)?].name))
        }
        NodeKind::FieldFormalParameter => Some(token_range(
            ast[ast.cast::<FieldFormalParameter>(node)?].name,
        )),
        NodeKind::ImportDirective
        | NodeKind::ImportPrefixReference
        | NodeKind::InstanceCreationExpression => Some(node_range(node)),
        NodeKind::Label => Some(token_range(ast[ast.cast::<Label>(node)?].name)),
        NodeKind::LabelReference => Some(token_range(ast[ast.cast::<LabelReference>(node)?].name)),
        NodeKind::LibraryDirective => Some(node_range(node)),
        NodeKind::MethodDeclaration => {
            Some(token_range(ast[ast.cast::<MethodDeclaration>(node)?].name))
        }
        NodeKind::ClassDeclaration => {
            let c = ast.cast::<ClassDeclaration>(node)?;
            Some(token_range(support::class_name_token(
                ast,
                ast[c].name_part,
            )))
        }
        NodeKind::EnumDeclaration => {
            let c = ast.cast::<EnumDeclaration>(node)?;
            Some(token_range(support::class_name_token(
                ast,
                ast[c].name_part,
            )))
        }
        NodeKind::ExtensionTypeDeclaration => {
            let c = ast.cast::<ExtensionTypeDeclaration>(node)?;
            Some(token_range(support::class_name_token(
                ast,
                ast[c].name_part,
            )))
        }
        NodeKind::FunctionDeclaration => Some(token_range(
            ast[ast.cast::<FunctionDeclaration>(node)?].name,
        )),
        NodeKind::MixinDeclaration => {
            Some(token_range(ast[ast.cast::<MixinDeclaration>(node)?].name))
        }
        NodeKind::FunctionTypeAlias => {
            Some(token_range(ast[ast.cast::<FunctionTypeAlias>(node)?].name))
        }
        NodeKind::GenericTypeAlias => {
            Some(token_range(ast[ast.cast::<GenericTypeAlias>(node)?].name))
        }
        NodeKind::ClassTypeAlias => Some(token_range(ast[ast.cast::<ClassTypeAlias>(node)?].name)),
        NodeKind::NamedType => Some(token_range(ast[ast.cast::<NamedType>(node)?].name)),
        NodeKind::NamedArgument => Some(token_range(ast[ast.cast::<NamedArgument>(node)?].name)),
        NodeKind::PrimaryConstructorName => Some(token_range(
            ast[ast.cast::<PrimaryConstructorName>(node)?].name,
        )),
        NodeKind::SimpleIdentifier => {
            Some(token_range(ast[ast.cast::<SimpleIdentifier>(node)?].token))
        }
        NodeKind::TypeParameter => Some(token_range(ast[ast.cast::<TypeParameter>(node)?].name)),
        NodeKind::VariableDeclaration => Some(token_range(
            ast[ast.cast::<VariableDeclaration>(node)?].name,
        )),
        _ if ast.is::<FormalParameter>(node) => {
            let p = ast.cast::<FormalParameter>(node)?;
            formal_parameter_name(ast, p).map(token_range)
        }
        _ => None,
    };
    let (mut name_offset, mut name_length) = range?;
    if import_index.is_some() {
        if let Some(i) = ast.cast::<ImportDirective>(node) {
            match ast[i].prefix {
                Some(p) => {
                    name_offset = ast.offset(p);
                    name_length = ast.length(p);
                }
                None => {
                    name_offset = u32::MAX;
                    name_length = 0;
                }
            }
        }
    }
    if let Some(i) = ast.cast::<InstanceCreationExpression>(node) {
        let named_type = ast[ast[i].constructor_name].type_;
        element = support::element_of(&ctx, &unit.tables, named_type);
        (name_offset, name_length) = token_range(ast[named_type].name);
    }
    let target = match import_index {
        Some(index)
            if element.is_none()
                || ast.is::<ImportDirective>(node)
                || ast.is::<SimpleIdentifier>(node) =>
        {
            Target::Import {
                unit: ResolvedUnitRefClone::of(resolved),
                index,
            }
        }
        _ => Target::Element(SElem {
            lib: resolved.library.clone(),
            unit: resolved.index,
            id: element?,
        }),
    };
    Some((target, name_offset, name_length))
}

pub fn create_rename_for_target(_resolved: &ResolvedUnitRef, target: Target) -> Option<Rename> {
    let new = |kind: Kind, target: Target, old_name: String| Rename {
        kind,
        target,
        old_name,
        new_name: String::new(),
        flutter_state: None,
        class_elements: Vec::new(),
        class_references: Vec::new(),
        class_unshadowed: Vec::new(),
        parameters: Vec::new(),
        rename_all_positional: false,
        extension_references: Vec::new(),
    };
    let element = match target {
        Target::Import { unit, index } => {
            let old_name = import_prefix_name(&unit, index);
            return Some(new(Kind::Import, Target::Import { unit, index }, old_name));
        }
        Target::Element(e) => e,
    };
    let id = element.with(|ctx| {
        let mut id = element.id;
        if matches!(id.tag(), Tag::Getter | Tag::Setter)
            && let Some(v) = dartr_resolver::element_metadata::accessor_variable_any(ctx, id)
        {
            id = v;
        }
        id
    });
    let element = element.same(id);
    let old_name = element.with(|ctx| old_name_of(ctx, id));
    let kind = element.with(|ctx| -> Option<Kind> {
        let mut enclosing = enclosing(ctx, id);
        if id.tag() == Tag::Prefix {
            enclosing = support::library_of(ctx, id).map(|l| l.raw());
        }
        if enclosing.is_some_and(|e| e.tag() == Tag::Library) {
            return Some(Kind::UnitMember);
        }
        match id.tag() {
            Tag::Constructor => return Some(Kind::Constructor),
            Tag::Label => return Some(Kind::Label),
            Tag::Library => return Some(Kind::Library),
            _ => {}
        }
        // `enclosingElement?.thisOrAncestorOfType<InterfaceElement>()`.
        let mut ancestor = enclosing;
        let mut interface = None;
        while let Some(a) = ancestor {
            if is_interface(a) {
                interface = Some(a);
                break;
            }
            ancestor = self::enclosing(ctx, a);
        }
        if interface.is_some() && id.tag() == Tag::FieldFormalParameter {
            if let dartr_element::AnyElement::FormalParameter(p) = ctx.any(id)
                && p.field.get().is_some()
            {
                return Some(Kind::ClassMember {
                    interface: interface.unwrap(),
                });
            }
        }
        if is_parameter(id) {
            return Some(Kind::Parameter);
        }
        if is_local(id) {
            return Some(Kind::Local);
        }
        if id.tag() == Tag::TypeParameter {
            return Some(Kind::TypeParameter);
        }
        if let Some(e) = enclosing.filter(|e| is_interface(*e)) {
            return Some(Kind::ClassMember { interface: e });
        }
        if let Some(e) = enclosing.filter(|e| e.tag() == Tag::Extension) {
            return Some(Kind::ExtensionMember { extension: e });
        }
        None
    })?;
    // A field formal parameter of a field renames the field.
    let element = if let Kind::ClassMember { .. } = kind
        && id.tag() == Tag::FieldFormalParameter
    {
        let field = element.with(|ctx| match ctx.any(id) {
            dartr_element::AnyElement::FormalParameter(p) => p.field.get().map(|f| f.raw()),
            _ => None,
        });
        match field {
            Some(f) => element.same(f),
            None => element,
        }
    } else {
        element
    };
    let old_name = if element.id != id {
        element.with(|ctx| old_name_of(ctx, element.id))
    } else {
        old_name
    };
    Some(new(kind, Target::Element(element), old_name))
}

impl Engine<'_> {
    // ---------------------------------------------------------------------
    // Conditions.

    /// Dart `checkInitialConditions`.
    pub fn check_initial_conditions(&mut self, rename: &mut Rename) -> RefactoringStatus {
        let mut result = RefactoringStatus::new();
        if rename.kind == Kind::UnitMember {
            self.find_flutter_state_class(rename);
        }
        if let Target::Element(e) = &rename.target {
            let (in_sdk, kind, qualified) = e.with(|ctx| {
                (
                    is_in_sdk(ctx, e.id),
                    kind_name(e.id),
                    self.qualified_name(ctx, e.id),
                )
            });
            if in_sdk {
                result.add_fatal_error(
                    format_list(
                        "The {0} '{1}' is defined in the SDK, so cannot be renamed.",
                        &[kind, &qualified],
                    ),
                    None,
                );
            }
            if !self.workspace_contains(&rename.target) {
                result.add_fatal_error(
                    format_list(
                        "The {0} '{1}' is defined outside of the project, so cannot be renamed.",
                        &[kind, &qualified],
                    ),
                    None,
                );
            }
            if matches!(
                rename.kind,
                Kind::ClassMember { .. } | Kind::ExtensionMember { .. }
            ) && e.id.tag() == Tag::Method
                && e.with(|ctx| is_operator(ctx, e.id))
            {
                result.add_fatal_error("Cannot rename operator.", None);
            }
        } else if !self.workspace_contains(&rename.target) {
            result.add_fatal_error(
                format_list(
                    "The {0} '{1}' is defined outside of the project, so cannot be renamed.",
                    &["import prefix", &rename.old_name],
                ),
                None,
            );
        }
        result
    }

    /// Dart `getElementQualifiedName`.
    fn qualified_name(&self, ctx: &Ctx<'_>, e: ElementId) -> String {
        match e.kind() {
            dartr_element::ElementKind::Field | dartr_element::ElementKind::Method => {
                let enclosing = enclosing(ctx, e)
                    .map(|c| display_name(ctx, c))
                    .unwrap_or_default();
                format!("{enclosing}.{}", display_name(ctx, e))
            }
            dartr_element::ElementKind::Library => {
                let path = element_path(ctx, e).unwrap_or_default();
                let root = self
                    .host
                    .collection()
                    .and_then(|c| c.context_for(&path))
                    .map(|c| c.root.root.clone())
                    .unwrap_or_default();
                relative_path(&path, &root)
            }
            _ => display_name(ctx, e),
        }
    }

    /// Dart `checkNewName`.
    pub fn check_new_name(&self, rename: &Rename) -> RefactoringStatus {
        let mut result = RefactoringStatus::new();
        let new_name = rename.new_name.as_str();
        if new_name == rename.old_name {
            result.add_fatal_error(
                "The new name must be different than the current name.",
                None,
            );
        }
        let tag = match &rename.target {
            Target::Element(e) => Some(e.id.tag()),
            Target::Import { .. } => None,
        };
        match &rename.kind {
            Kind::Import => result.add_status(naming::validate_import_prefix_name(new_name)),
            Kind::Local => match tag {
                Some(Tag::LocalFunction) => {
                    result.add_status(naming::validate_function_name(new_name))
                }
                Some(Tag::LocalVariable) => {
                    result.add_status(naming::validate_variable_name(new_name))
                }
                _ => {}
            },
            Kind::Parameter => result.add_status(naming::validate_parameter_name(new_name)),
            Kind::Label => result.add_status(naming::validate_label_name(new_name)),
            Kind::Library => result.add_status(naming::validate_library_name(new_name)),
            Kind::TypeParameter => result.add_status(naming::validate_type_parameter(new_name)),
            Kind::UnitMember => {
                let e = match &rename.target {
                    Target::Element(e) => e.id,
                    _ => unreachable!(),
                };
                if e.tag() == Tag::ExtensionType {
                    result.add_status(naming::validate_extension_type_name(new_name));
                }
                if e.tag() == Tag::TopLevelFunction {
                    result.add_status(naming::validate_function_name(new_name));
                }
                if is_interface(e) {
                    result.add_status(naming::validate_class_name(new_name));
                }
                if e.tag() == Tag::TopLevelVariable {
                    result.add_status(naming::validate_variable_name(new_name));
                }
                if e.tag() == Tag::TypeAlias {
                    result.add_status(naming::validate_type_alias_name(new_name));
                }
            }
            Kind::ClassMember { .. } | Kind::ExtensionMember { .. } => match tag {
                Some(Tag::Field) => result.add_status(naming::validate_field_name(new_name)),
                Some(Tag::Method) => result.add_status(naming::validate_method_name(new_name)),
                _ => {}
            },
            Kind::Constructor => {
                result.add_status(naming::validate_constructor_name(new_name));
                if let Target::Element(e) = &rename.target {
                    e.with(|ctx| {
                        let Some(class) = enclosing(ctx, e.id) else { return };
                        if ctx.element_name(class) == Some(new_name) {
                            result.add_error(
                                "The constructor should not have the same name as the name of the enclosing class.",
                                None,
                            );
                        }
                        for member in children_named(ctx, class, new_name) {
                            result.add_error(
                                format_list(
                                    "{0} '{1}' already declares {2} with name '{3}'.",
                                    &[
                                        &capitalize(kind_name(class)),
                                        &display_name(ctx, class),
                                        kind_name(member),
                                        new_name,
                                    ],
                                ),
                                None,
                            );
                        }
                    });
                }
            }
        }
        result
    }

    /// Dart `checkFinalConditions`.
    pub fn check_final_conditions(&mut self, rename: &mut Rename) -> RefactoringStatus {
        match rename.kind.clone() {
            Kind::Import => {
                let mut result = RefactoringStatus::new();
                if rename.new_name.is_empty()
                    && let Target::Import { unit, index } = &rename.target
                {
                    let r = unit.unit_ref();
                    let u = r.unit();
                    let ast = &u.ast;
                    if let Some(i) = nth_import(ast, u.unit, *index)
                        && ast[i].deferred_keyword.is_some()
                    {
                        result.add_status(RefactoringStatus::error(
                            "Deferred imports require a prefix",
                        ));
                    }
                }
                result
            }
            Kind::Local => {
                let mut result = RefactoringStatus::new();
                if let Target::Element(e) = rename.target.clone() {
                    self.conflict_validate(&e, &rename.new_name, &mut result);
                }
                result
            }
            Kind::Label | Kind::Library | Kind::Constructor => RefactoringStatus::new(),
            Kind::TypeParameter => self.check_type_parameter(rename),
            Kind::Parameter => self.check_parameter(rename),
            Kind::UnitMember => self.check_unit_member(rename),
            Kind::ClassMember { interface } => self.check_class_member(rename, interface),
            Kind::ExtensionMember { extension } => self.check_extension_member(rename, extension),
        }
    }

    fn check_type_parameter(&mut self, rename: &Rename) -> RefactoringStatus {
        let mut result = RefactoringStatus::new();
        let Target::Element(e) = &rename.target else {
            return result;
        };
        e.with(|ctx| {
            let Some(enclosing) = enclosing(ctx, e.id) else {
                return;
            };
            for sibling in type_parameters_of(ctx, enclosing) {
                if ctx.element_name(sibling) == Some(rename.new_name.as_str()) {
                    result.add_error(
                        format!("Duplicate {} '{}'.", kind_name(sibling), rename.new_name),
                        None,
                    );
                }
            }
        });
        result
    }

    // ---------------------------------------------------------------------
    // Parameters.

    /// Dart `RenameParameterRefactoringImpl.checkFinalConditions`.
    fn check_parameter(&mut self, rename: &mut Rename) -> RefactoringStatus {
        let mut result = RefactoringStatus::new();
        let mut conflict_result = RefactoringStatus::new();
        let Target::Element(e) = rename.target.clone() else {
            return result;
        };
        let named = e.with(|ctx| parameter_is_named(ctx, e.id));
        rename.parameters = if named {
            let mut ps = self.hierarchy_named_parameters(&e);
            self.add_named_super_formal_parameters(&mut ps);
            ps
        } else {
            self.hierarchy_positional_parameters(&e)
        };
        for p in rename.parameters.clone() {
            let (p_named, p_name) =
                p.with(|ctx| (parameter_is_named(ctx, p.id), display_name(ctx, p.id)));
            if rename.new_name.starts_with('_') && p_named {
                result.add_error(
                    format_list(
                        "The parameter '{0}' is named and can not be private.",
                        &[&p_name],
                    ),
                    None,
                );
                break;
            }
            if !p_named && !rename.rename_all_positional {
                let path = p.with(|ctx| element_path(ctx, p.id));
                if let Some(path) = path
                    && self.lint_enabled(&path, "avoid_renaming_method_parameters")
                {
                    rename.rename_all_positional = true;
                }
            }
            let new_name = rename.new_name.clone();
            self.conflict_validate(&p, &new_name, &mut conflict_result);
        }
        if rename.rename_all_positional && rename.parameters.len() > 1 {
            result.add_status(status_from(
                "This will also rename all related positional parameters to the same name.",
                &conflict_result,
            ));
        }
        if result.problem().is_none() {
            return conflict_result;
        }
        result
    }

    /// Whether the lint [name] is enabled for [path].
    fn lint_enabled(&self, path: &str, name: &str) -> bool {
        let Some(collection) = self.host.collection() else {
            return false;
        };
        let Some(context) = collection.context_for(path) else {
            return false;
        };
        collection
            .options_for(context, path)
            .lint_rules
            .iter()
            .any(|r| r == name)
    }

    /// Dart `addNamedSuperFormalParameters`.
    fn add_named_super_formal_parameters(&mut self, parameters: &mut Vec<SElem>) {
        let mut i = 0;
        while i < parameters.len() {
            let p = parameters[i].clone();
            if p.with(|ctx| parameter_is_named(ctx, p.id)) {
                for m in self.search_references(&p) {
                    if let Some(e) = self.match_element(&m)
                        && e.id.tag() == Tag::SuperFormalParameter
                    {
                        parameters.push(e);
                    }
                }
            }
            i += 1;
        }
    }

    /// Dart `getHierarchyPositionalParameters`.
    fn hierarchy_positional_parameters(&mut self, element: &SElem) -> Vec<SElem> {
        let method =
            element.with(|ctx| enclosing(ctx, element.id).filter(|m| m.tag() == Tag::Method));
        let Some(method) = method else {
            return vec![element.clone()];
        };
        let index = element.with(|ctx| positional_index(ctx, method, element.id));
        let Some(index) = index else {
            return vec![element.clone()];
        };
        let (members, _) = self.hierarchy_members_and_parameters(&element.same(method));
        let mut out = Vec::new();
        for m in members {
            if m.id.tag() != Tag::Method {
                continue;
            }
            let found = m.with(|ctx| {
                let params = positional_parameters(ctx, m.id);
                params.get(index).copied()
            });
            if let Some(p) = found {
                out.push(m.same(p));
            }
        }
        out
    }

    // ---------------------------------------------------------------------
    // Search helpers.

    /// Dart `SearchMatch.element`: the element whose fragment encloses the
    /// match (Dart `_getEnclosingFragment`).
    pub fn match_element(&mut self, m: &Match) -> Option<SElem> {
        let resolved = self
            .require_resolved_unit_in(&m.path, Some(m.context))
            .ok()?;
        let unit = resolved.unit();
        let ast = &unit.ast;
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let id =
            enclosing_declaration(&ctx, ast, &unit.tables, unit.unit, m.offset).or_else(|| {
                // The library fragment: the library.
                Some(resolved.library.library.library.raw())
            })?;
        Some(SElem {
            lib: resolved.library.clone(),
            unit: resolved.index,
            id,
        })
    }
}

/// Dart `RenameRefactoringImpl._getOldName`.
fn old_name_of(ctx: &Ctx<'_>, e: ElementId) -> String {
    if e.tag() == Tag::Constructor {
        let name = ctx.element_name(e);
        return match name {
            None | Some("new") => String::new(),
            Some(n) => n.to_string(),
        };
    }
    display_name(ctx, e)
}

/// The name of the prefix of the import [index] of [unit] (empty without
/// a prefix).
fn import_prefix_name(unit: &ResolvedUnitRefClone, index: usize) -> String {
    let r = unit.unit_ref();
    let u = r.unit();
    let ast = &u.ast;
    nth_import(ast, u.unit, index)
        .and_then(|i| ast[i].prefix)
        .map(|p| ast.tokens.lexeme(ast[p].token).to_string())
        .unwrap_or_default()
}

fn nth_import(ast: &Ast, unit: Id<CompilationUnit>, index: usize) -> Option<Id<ImportDirective>> {
    ast.list_raw(ast[unit].directives)
        .iter()
        .filter_map(|d| ast.cast::<ImportDirective>(*d))
        .nth(index)
}

pub(crate) fn import_index_of(
    ast: &Ast,
    unit: Id<CompilationUnit>,
    import: Id<ImportDirective>,
) -> Option<usize> {
    ast.list_raw(ast[unit].directives)
        .iter()
        .filter_map(|d| ast.cast::<ImportDirective>(*d))
        .position(|i| i == import)
}

/// Dart `getImportElement(prefixNode)`: the import that declares the
/// prefix [node] and imports the element it qualifies.
fn prefixed_import_index(
    ctx: &Ctx<'_>,
    ast: &Ast,
    unit: &dartr_resolver::library_analyzer::ResolvedUnit,
    resolved: &ResolvedUnitRef,
    node: Id<SimpleIdentifier>,
) -> Option<usize> {
    let parent = ast.parent(node)?;
    if let Some(i) = ast.cast::<ImportDirective>(parent) {
        return import_index_of(ast, unit.unit, i);
    }
    let used = if let Some(p) = ast.cast::<PrefixedIdentifier>(parent) {
        if ast[p].prefix != node {
            return None;
        }
        support::element_of(ctx, &unit.tables, p)
            .or_else(|| support::element_of(ctx, &unit.tables, ast[p].identifier))
    } else if let Some(m) = ast.cast::<MethodInvocation>(parent) {
        if ast[m].target.map(|t| t.raw()) != Some(node.raw()) {
            return None;
        }
        support::element_of(ctx, &unit.tables, ast[m].method_name)
    } else {
        None
    }?;
    let prefix = ast.tokens.lexeme(ast[node].token).to_string();
    // `_getImportElement`.
    if !enclosing(ctx, used).is_some_and(|e| e.tag() == Tag::Library) {
        return None;
    }
    let used_library = support::library_of(ctx, used)?;
    let first = ctx.get(resolved.library.library.library).first_fragment();
    let imports = &ctx.fragment(first).library_imports;
    let mut candidates: Vec<usize> = Vec::new();
    for (i, import) in imports.iter().enumerate() {
        let dartr_element::DirectiveUri::Library { library, .. } = &import.directive.uri else {
            continue;
        };
        if *library != used_library {
            continue;
        }
        let Some(p) = import.prefix else { continue };
        let prefix_name = ctx
            .fragment(p)
            .element
            .try_get()
            .and_then(|e| ctx.element_name(*e).map(str::to_string));
        if prefix_name.as_deref() != Some(prefix.as_str()) {
            continue;
        }
        if import.combinators.is_empty() {
            return Some(i);
        }
        candidates.push(i);
    }
    if candidates.len() == 1 {
        return Some(candidates[0]);
    }
    for i in candidates {
        if let Some((_, names)) = crate::completion::declaration::import_namespace(ctx, &imports[i])
            && names.iter().any(|(_, e)| *e == used)
        {
            return Some(i);
        }
    }
    None
}

/// The name token of the formal parameter [p].
fn formal_parameter_name(ast: &Ast, p: Id<FormalParameter>) -> Option<dartr_syntax::TokenId> {
    let p = p.raw();
    if let Some(r) = ast.cast::<RegularFormalParameter>(p) {
        return ast[r].name;
    }
    if let Some(f) = ast.cast::<FieldFormalParameter>(p) {
        return Some(ast[f].name);
    }
    ast.cast::<SuperFormalParameter>(p).map(|s| ast[s].name)
}

/// Whether [e] is an operator (Dart `MethodElement.isOperator`).
fn is_operator(ctx: &Ctx<'_>, e: ElementId) -> bool {
    ctx.element_name(e).is_some_and(|n| {
        n.chars()
            .next()
            .is_some_and(|c| !(c.is_alphabetic() || c == '_' || c == '$'))
    })
}

/// The type parameters of [e] (Dart `TypeParameterizedElement.typeParameters`).
fn type_parameters_of(ctx: &Ctx<'_>, e: ElementId) -> Vec<ElementId> {
    member::type_parameters(ctx, ElemRef::Base(e))
        .into_iter()
        .map(|t| t.raw())
        .collect()
}

/// The positional parameters of the executable [e].
fn positional_parameters(ctx: &Ctx<'_>, e: ElementId) -> Vec<ElementId> {
    let Some(x) = e.cast::<dartr_element::ExecutableElement>() else {
        return Vec::new();
    };
    ctx.executable(x)
        .formal_params
        .iter()
        .filter(|p| !ctx.get(**p).kind.is_named())
        .map(|p| p.raw())
        .collect()
}

fn positional_index(ctx: &Ctx<'_>, method: ElementId, parameter: ElementId) -> Option<usize> {
    positional_parameters(ctx, method)
        .iter()
        .position(|p| *p == parameter)
}

/// Dart `_RefactoringStatusExt.from`.
fn status_from(message: &str, result: &RefactoringStatus) -> RefactoringStatus {
    let aggregated = if result.problems.is_empty() {
        String::new()
    } else if result.problems.len() == 1 {
        format!("\n{}", result.problems[0].message)
    } else {
        format!(
            "\n{} And {} more error{}.",
            result.problems[0].message,
            result.problems.len() - 1,
            if result.problems.len() > 1 { "s" } else { "" }
        )
    };
    let message = format!("{message}{aggregated}");
    use super::status::Severity;
    match result.severity() {
        Some(Severity::Error) => RefactoringStatus::error(message),
        Some(Severity::Fatal) => RefactoringStatus::fatal(message),
        _ => RefactoringStatus::warning(message),
    }
}

/// `path.relative(path, from: root)`.
fn relative_path(path: &str, root: &str) -> String {
    let root = root.trim_end_matches('/');
    match path.strip_prefix(&format!("{root}/")) {
        Some(r) => r.to_string(),
        None => path.to_string(),
    }
}

/// Dart `_getEnclosingFragment(libraryFragment, offset).element`: the
/// innermost declaration of the element model (top-level declarations,
/// their members, and the formal parameters of those) whose code range
/// contains [offset].
fn enclosing_declaration(
    ctx: &Ctx<'_>,
    ast: &Ast,
    tables: &dartr_element::ResolutionTables,
    root: Id<CompilationUnit>,
    offset: u32,
) -> Option<ElementId> {
    let node = ast.node_covering(root, offset, 0)?;
    let mut chain = vec![node];
    let mut n = node;
    while let Some(p) = ast.parent(n) {
        chain.push(p);
        n = p;
    }
    chain.reverse();
    let mut found: Option<ElementId> = None;
    for n in chain {
        if ast.is::<CompilationUnit>(n) {
            continue;
        }
        let is_declaration = ast.is::<CompilationUnitMember>(n)
            || ast.is::<ClassMember>(n)
            || ast.is::<EnumConstantDeclaration>(n)
            || ast.is::<VariableDeclaration>(n)
            || ast.is::<FormalParameter>(n)
            || ast.is::<TypeParameter>(n);
        if is_declaration {
            if let Some(e) = support::declared_element(ctx, tables, n) {
                found = Some(e);
            }
            continue;
        }
        if ast.is::<FunctionBody>(n)
            || ast.is::<Statement>(n)
            || (ast.is::<Expression>(n) && !ast.is::<FunctionExpression>(n))
        {
            break;
        }
    }
    found
}

/// Dart `getSourceReferences`: the distinct references (file and range) in
/// the order of a Dart `HashMap`.
pub(crate) fn source_references(matches: &[Match]) -> Vec<Match> {
    let mut unique: Vec<Match> = Vec::new();
    for m in matches {
        if !unique
            .iter()
            .any(|u| u.path == m.path && u.offset == m.offset && u.length == m.length)
        {
            unique.push(m.clone());
        }
    }
    let hashes: Vec<u32> = unique
        .iter()
        .map(|m| {
            // `SourceReference.hashCode`.
            let file = crate::server::dart_string_hash(&m.path) as u64;
            let range = (31u64.wrapping_mul(m.offset as u64)).wrapping_add(m.length as u64);
            (((file << 16) & 0xFFFF_FFFF).wrapping_add(range)) as u32
        })
        .collect();
    crate::server::dart_hash_map_order(&hashes)
        .into_iter()
        .map(|i| unique[i].clone())
        .collect()
}

// -------------------------------------------------------------------------
// Unit members, class members and extension members.

/// Dart `InterfaceElement.allSupertypes` elements.
fn all_supertype_elements(ctx: &Ctx<'_>, interface: ElementId) -> Vec<ElementId> {
    let Some(i) = interface.cast::<dartr_element::InterfaceElement>() else {
        return Vec::new();
    };
    dartr_typesystem::class_hierarchy::implemented_interfaces(ctx, i)
        .iter()
        .filter_map(|&t| match ctx.ty(t) {
            dartr_element::TypeKind::Interface { element, .. } => Some(element.raw()),
            _ => None,
        })
        .collect()
}

/// `thisOrAncestorOfType<InterfaceElement>()`.
fn interface_ancestor(ctx: &Ctx<'_>, e: ElementId) -> Option<ElementId> {
    let mut current = Some(e);
    while let Some(c) = current {
        if is_interface(c) {
            return Some(c);
        }
        current = enclosing(ctx, c);
    }
    None
}

/// Dart `Element.children` of an interface or extension (the direct
/// children: fields, getters, setters, methods and constructors).
fn instance_children(ctx: &Ctx<'_>, parent: ElementId) -> Vec<ElementId> {
    let Some(instance) = parent.cast::<dartr_element::InstanceElement>() else {
        return Vec::new();
    };
    let data = ctx.instance(instance);
    let mut children: Vec<ElementId> = data
        .fields
        .iter()
        .map(|f| f.raw())
        .chain(data.getters.iter().map(|g| g.raw()))
        .chain(data.setters.iter().map(|s| s.raw()))
        .chain(data.methods.iter().map(|m| m.raw()))
        .collect();
    if let Some(interface) = parent.cast::<dartr_element::InterfaceElement>() {
        children.extend(
            ctx.interface(interface)
                .constructors
                .iter()
                .map(|c| c.raw()),
        );
    }
    children
}

/// Dart `isStatefulWidgetDeclaration`.
fn is_stateful_widget_declaration(ctx: &Ctx<'_>, class: ElementId) -> bool {
    let Some(i) = class.cast::<dartr_element::InterfaceElement>() else {
        return false;
    };
    if class.tag() != Tag::Class {
        return false;
    }
    let Some(supertype) = ctx.interface(i).supertype.get() else {
        return false;
    };
    match ctx.ty(supertype) {
        dartr_element::TypeKind::Interface { element, .. } => {
            ctx.element_name(element.raw()) == Some("StatefulWidget")
                && support::library_of(ctx, element.raw()).is_some_and(|l| {
                    ctx.library_uri(l) == "package:flutter/src/widgets/framework.dart"
                })
        }
        _ => false,
    }
}

impl Engine<'_> {
    /// Dart `_findFlutterStateClass`.
    fn find_flutter_state_class(&mut self, rename: &mut Rename) {
        let Target::Element(e) = &rename.target else {
            return;
        };
        let found = e.with(|ctx| {
            if !is_stateful_widget_declaration(ctx, e.id) {
                return None;
            }
            let library = support::library_of(ctx, e.id)?;
            let state = format!("{}State", rename.old_name);
            let find = |name: &str| {
                ctx.get(library)
                    .classes
                    .iter()
                    .map(|c| c.raw())
                    .find(|c| ctx.element_name(*c) == Some(name))
            };
            find(&state).or_else(|| find(&format!("_{state}")))
        });
        rename.flutter_state = found.map(|s| e.same(s));
    }

    /// Dart `_updateFlutterWidgetStateName`.
    fn flutter_state_new_name(rename: &Rename) -> Option<String> {
        let state = rename.flutter_state.as_ref()?;
        let name = state.with(|ctx| display_name(ctx, state.id));
        let mut new_name = format!("{}State", rename.new_name);
        if name.starts_with('_') && !new_name.starts_with('_') {
            new_name = format!("_{new_name}");
        }
        Some(new_name)
    }

    /// Dart `RenameUnitMemberRefactoringImpl.checkFinalConditions`.
    fn check_unit_member(&mut self, rename: &mut Rename) -> RefactoringStatus {
        let Target::Element(e) = rename.target.clone() else {
            return RefactoringStatus::new();
        };
        let mut status = self.validate_rename_top_level(&e, &rename.new_name);
        if let (Some(state), Some(new_name)) = (
            rename.flutter_state.clone(),
            Self::flutter_state_new_name(rename),
        ) {
            status.add_status(self.validate_rename_top_level(&state, &new_name));
        }
        status
    }

    /// Dart `_RenameUnitMemberValidator.validate`.
    fn validate_rename_top_level(&mut self, element: &SElem, name: &str) -> RefactoringStatus {
        let mut result = RefactoringStatus::new();
        let element_kind = kind_name(element.id);
        // `_validateWillConflict`.
        element.with(|ctx| {
            let Some(library) = support::library_of(ctx, element.id) else {
                return;
            };
            for child in crate::server::library_children(ctx, library) {
                if display_name(ctx, child) == name {
                    result.add_error(
                        format_list(
                            "Library already declares {0} with name '{1}'.",
                            &[kind_name(child), name],
                        ),
                        None,
                    );
                }
            }
        });
        let references = self.search_references(element);
        let library_key = element.with(|ctx| {
            support::library_of(ctx, element.id).and_then(|l| element_path(ctx, l.raw()))
        });
        // `_validateWillBeInvisible`.
        if name.starts_with('_') {
            for reference in &references {
                let Some(ref_element) = self.match_element(reference) else {
                    continue;
                };
                let ref_library = ref_element.with(|ctx| support::library_of(ctx, ref_element.id));
                let ref_library_path =
                    ref_element.with(|ctx| ref_library.and_then(|l| element_path(ctx, l.raw())));
                if ref_library_path != library_key {
                    let qualified = ref_element.with(|ctx| {
                        ref_library
                            .map(|l| self.qualified_name(ctx, l.raw()))
                            .unwrap_or_default()
                    });
                    result.add_error(
                        format_list(
                            "Renamed {0} will be invisible in '{1}'.",
                            &[element_kind, &qualified],
                        ),
                        None,
                    );
                }
            }
        }
        // `_validateWillBeShadowed`.
        for reference in &references {
            let Some(ref_element) = self.match_element(reference) else {
                continue;
            };
            ref_element.with(|ctx| {
                let Some(ref_class) = interface_ancestor(ctx, ref_element.id) else {
                    return;
                };
                for shadow in instance_children(ctx, ref_class) {
                    if display_name(ctx, shadow) == name {
                        result.add_error(
                            format_list(
                                "Reference to renamed {0} will be shadowed by {1} '{2}'.",
                                &[
                                    element_kind,
                                    kind_name(shadow),
                                    &self.qualified_name(ctx, shadow),
                                ],
                            ),
                            None,
                        );
                    }
                }
            });
        }
        // `_validateWillShadow`.
        self.validate_will_shadow(&mut result, Some(element), element_kind, name, true);
        result
    }

    /// Dart `_BaseUnitMemberValidator._validateWillShadow`.
    fn validate_will_shadow(
        &mut self,
        result: &mut RefactoringStatus,
        element: Option<&SElem>,
        element_kind: &str,
        name: &str,
        is_rename: bool,
    ) {
        let declarations = self.search_member_declarations(name);
        for member in declarations {
            let declaring_class = member.with(|ctx| enclosing(ctx, member.id));
            let declaring_key =
                member.with(|ctx| declaring_class.and_then(|c| crate::index::element_key(ctx, c)));
            let member_references = self.search_references(&member);
            for reference in member_references {
                if reference.is_qualified {
                    continue;
                }
                let Some(ref_element) = self.match_element(&reference) else {
                    continue;
                };
                let ref_class_key = ref_element.with(|ctx| {
                    interface_ancestor(ctx, ref_element.id)
                        .and_then(|c| crate::index::element_key(ctx, c))
                });
                if ref_class_key.is_some() && ref_class_key == declaring_key {
                    continue;
                }
                if let Some(element) = element
                    && !self.is_visible_at(element, &ref_element)
                {
                    continue;
                }
                let (kind, qualified) =
                    member.with(|ctx| (kind_name(member.id), self.qualified_name(ctx, member.id)));
                result.add_error(
                    format_list(
                        if is_rename {
                            "Renamed {0} will shadow {1} '{2}'."
                        } else {
                            "Created {0} will shadow {1} '{2}'."
                        },
                        &[element_kind, kind, &qualified],
                    ),
                    None,
                );
            }
        }
    }

    /// Dart `_BaseUnitMemberValidator._isVisibleAt`.
    fn is_visible_at(&mut self, element: &SElem, at: &SElem) -> bool {
        let library = element.with(|ctx| {
            support::library_of(ctx, element.id).and_then(|l| element_path(ctx, l.raw()))
        });
        let at_library = at.with(|ctx| support::library_of(ctx, at.id));
        let at_library_path = at.with(|ctx| at_library.and_then(|l| element_path(ctx, l.raw())));
        if library.is_some() && library == at_library_path {
            return true;
        }
        let key = element.key();
        at.with(|ctx| {
            let Some(at_library) = at_library else {
                return false;
            };
            let first = ctx.get(at_library).first_fragment();
            for import in &ctx.fragment(first).library_imports {
                if import.prefix.is_some() {
                    continue;
                }
                if let Some((_, names)) =
                    crate::completion::declaration::import_namespace(ctx, import)
                    && names
                        .iter()
                        .any(|(_, e)| crate::index::element_key(ctx, *e) == key && key.is_some())
                {
                    return true;
                }
            }
            false
        })
    }

    /// Dart `SearchEngine.searchMemberDeclarations(name)` (`classMembers`):
    /// the getters, setters, fields and methods named [name] of the
    /// classes, enums, extension types and mixins of the searched files.
    fn search_member_declarations(&mut self, name: &str) -> Vec<SElem> {
        let scope = self.search_scope();
        let mut out = Vec::new();
        let names = vec![name.to_string()];
        for (context, files) in scope.iter() {
            let mut checked: Vec<String> = Vec::new();
            for f in files {
                if !self.mentions_any(f, &names) {
                    continue;
                }
                let Ok(resolved) = self.require_resolved_unit_in(f, Some(*context)) else {
                    continue;
                };
                let library_path = resolved.library.inputs[0].path.to_string();
                if checked.contains(&library_path) {
                    continue;
                }
                checked.push(library_path);
                let sink = NoopSink;
                let ctx = resolved.ctx(&sink);
                let library = ctx.get(resolved.library.library.library);
                let interfaces: Vec<ElementId> = library
                    .classes
                    .iter()
                    .map(|c| c.raw())
                    .chain(library.enums.iter().map(|c| c.raw()))
                    .chain(library.extension_types.iter().map(|c| c.raw()))
                    .chain(library.mixins.iter().map(|c| c.raw()))
                    .collect();
                for interface in interfaces {
                    let Some(instance) = interface.cast::<dartr_element::InstanceElement>() else {
                        continue;
                    };
                    let data = ctx.instance(instance);
                    let origin = |e: ElementId| {
                        let flags = dartr_resolver::element_ext::first_fragment_flags(&ctx, e);
                        !flags
                            .contains(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE)
                    };
                    let field_origin = |e: ElementId| {
                        let flags = dartr_resolver::element_ext::first_fragment_flags(&ctx, e);
                        flags.contains(
                            FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_DECLARATION,
                        ) || flags.contains(
                            FragmentFlags::FIELD_FRAGMENT_IS_ORIGIN_DECLARING_FORMAL_PARAMETER,
                        )
                    };
                    let mut members: Vec<ElementId> = Vec::new();
                    members.extend(data.getters.iter().map(|g| g.raw()).filter(|g| origin(*g)));
                    members.extend(data.setters.iter().map(|g| g.raw()).filter(|g| origin(*g)));
                    members.extend(
                        data.fields
                            .iter()
                            .map(|g| g.raw())
                            .filter(|g| field_origin(*g)),
                    );
                    members.extend(data.methods.iter().map(|g| g.raw()));
                    for m in members {
                        if display_name(&ctx, m) == name {
                            out.push(SElem {
                                lib: resolved.library.clone(),
                                unit: resolved.index,
                                id: m,
                            });
                        }
                    }
                }
            }
        }
        out
    }

    /// Dart `RenameClassMemberRefactoringImpl.checkFinalConditions`
    /// (`_RenameClassMemberValidator.validate`).
    fn check_class_member(
        &mut self,
        rename: &mut Rename,
        interface: ElementId,
    ) -> RefactoringStatus {
        let mut result = RefactoringStatus::new();
        let Target::Element(element) = rename.target.clone() else {
            return result;
        };
        let name = rename.new_name.clone();
        let element_kind = kind_name(element.id);
        let interface_s = element.same(interface);
        // `_checkClassAlreadyDeclares`.
        element.with(|ctx| {
            for member in children_named(ctx, interface, &name) {
                result.add_error(
                    format_list(
                        "{0} '{1}' already declares {2} with name '{3}'.",
                        &[
                            &capitalize(kind_name(interface)),
                            &display_name(ctx, interface),
                            kind_name(member),
                            &name,
                        ],
                    ),
                    None,
                );
            }
        });
        // `_prepareReferences`.
        let mut elements: Vec<SElem> = if matches!(element.id.tag(), Tag::Field | Tag::Method) {
            self.hierarchy_members_and_parameters(&element).0
        } else {
            vec![element.clone()]
        };
        if elements.is_empty() {
            elements.push(element.clone());
        }
        let mut i = 0;
        while i < elements.len() {
            let e = elements[i].clone();
            i += 1;
            if e.id.tag() != Tag::Field {
                continue;
            }
            let mut formal_parameters: Vec<SElem> = e.with(|ctx| {
                let Some(class) =
                    enclosing(ctx, e.id).and_then(|c| c.cast::<dartr_element::InterfaceElement>())
                else {
                    return Vec::new();
                };
                let mut out = Vec::new();
                for &c in &ctx.interface(class).constructors {
                    for &p in &ctx.executable(c.upcast()).formal_params {
                        if p.raw().tag() == Tag::FieldFormalParameter
                            && let dartr_element::AnyElement::FormalParameter(fp) = ctx.any(p.raw())
                            && fp.field.get().map(|f| f.raw()) == Some(e.id)
                        {
                            out.push(e.same(p.raw()));
                        }
                    }
                }
                out
            });
            if name.starts_with('_') && !self.supports_private_named_parameters(&e) {
                formal_parameters.retain(|p| !p.with(|ctx| parameter_is_named(ctx, p.id)));
            }
            self.add_named_super_formal_parameters(&mut formal_parameters);
            elements.extend(formal_parameters);
        }
        let mut references: Vec<Match> = Vec::new();
        for e in &elements {
            references.extend(self.search_references(e));
        }
        // `appendAllSubtypes`.
        let mut sub_classes: Vec<SElem> = Vec::new();
        let mut keys = Vec::new();
        self.append_all_subtypes(&interface_s, &mut sub_classes, &mut keys);
        for e in &elements {
            e.with(|ctx| {
                if let Some(enclosing) = enclosing(ctx, e.id).filter(|c| is_interface(*c))
                    && ctx.element_name(enclosing) == Some(name.as_str())
                {
                    result.add_error(
                        format!(
                            "Renamed {element_kind} has the same name as the declaring {} '{name}'.",
                            kind_name(enclosing)
                        ),
                        None,
                    );
                }
            });
        }
        // `_getLocalShadowingElement`.
        let (conflict, unshadowed) =
            self.local_shadowing_element(&references, &name, element.id.tag() == Tag::Field);
        if let Some((kind, local_name)) = conflict {
            result.add_error(
                format_list(
                    "Usage of renamed {0} will be shadowed by {1} '{2}'.",
                    &[element_kind, kind, &local_name],
                ),
                None,
            );
        }
        // `_checkHierarchy`.
        let super_keys: Vec<_> = element.with(|ctx| {
            all_supertype_elements(ctx, interface)
                .into_iter()
                .filter_map(|s| crate::index::element_key(ctx, s))
                .collect()
        });
        let sub_keys: Vec<_> = sub_classes.iter().filter_map(|s| s.key()).collect();
        for declaration in self.search_member_declarations(&name) {
            let (name_element, name_class_key, kind, qualified) = declaration.with(|ctx| {
                let mut ne = declaration.id;
                if matches!(ne.tag(), Tag::Getter | Tag::Setter) {
                    let flags = dartr_resolver::element_ext::first_fragment_flags(ctx, ne);
                    if flags.contains(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE)
                        && let Some(v) =
                            dartr_resolver::element_metadata::accessor_variable_any(ctx, ne)
                    {
                        ne = v;
                    }
                }
                let class_key = enclosing(ctx, ne).and_then(|c| crate::index::element_key(ctx, c));
                (ne, class_key, kind_name(ne), self.qualified_name(ctx, ne))
            });
            let _ = name_element;
            if name_class_key
                .as_ref()
                .is_some_and(|k| super_keys.contains(k))
            {
                result.add_error(
                    format_list(
                        "Renamed {0} will shadow {1} '{2}'.",
                        &[element_kind, kind, &qualified],
                    ),
                    None,
                );
            }
            if name_class_key
                .as_ref()
                .is_some_and(|k| sub_keys.iter().any(|s| s == k))
            {
                result.add_error(
                    format_list(
                        "Renamed {0} will be shadowed by {1} '{2}'.",
                        &[element_kind, kind, &qualified],
                    ),
                    None,
                );
            }
        }
        // `_validateWillBeInvisible`.
        if name.starts_with('_') {
            let library = element.with(|ctx| {
                support::library_of(ctx, interface).and_then(|l| element_path(ctx, l.raw()))
            });
            for reference in &references {
                if reference.kind == RelationKind::IsReferencedByNamedArgument {
                    continue;
                }
                let Some(ref_element) = self.match_element(reference) else {
                    continue;
                };
                let (ref_library, qualified) = ref_element.with(|ctx| {
                    let l = support::library_of(ctx, ref_element.id);
                    (
                        l.and_then(|l| element_path(ctx, l.raw())),
                        l.map(|l| self.qualified_name(ctx, l.raw()))
                            .unwrap_or_default(),
                    )
                });
                if ref_library != library {
                    result.add_error(
                        format_list(
                            "Renamed {0} will be invisible in '{1}'.",
                            &[element_kind, &qualified],
                        ),
                        None,
                    );
                }
            }
        }
        rename.class_elements = elements;
        rename.class_references = references;
        rename.class_unshadowed = unshadowed;
        result
    }

    /// Whether the library of [e] has the feature `private-named-parameters`.
    fn supports_private_named_parameters(&self, e: &SElem) -> bool {
        e.with(|ctx| ctx.features.is_enabled("private-named-parameters"))
    }

    /// Dart `_getLocalShadowingElement`: the first local element named
    /// [name] whose visible range contains an unqualified reference (kind
    /// and name), and the references to the locals that can avoid the
    /// shadow of a field (as the enclosing element of each: path, offset,
    /// length of the reference).
    fn local_shadowing_element(
        &mut self,
        references: &[Match],
        name: &str,
        is_field: bool,
    ) -> (
        Option<(&'static str, String)>,
        Vec<(usize, crate::index::ElementKey)>,
    ) {
        let mut cache: HashMap<String, (Vec<LocalElement>, ResolvedUnitRefClone)> = HashMap::new();
        let mut unshadowed = Vec::new();
        for m in references {
            if m.is_qualified {
                continue;
            }
            let Some(match_element) = self.match_element(m) else {
                continue;
            };
            let path = match_element.with(|ctx| element_path(ctx, match_element.id));
            let Some(path) = path else { continue };
            if !cache.contains_key(&path) {
                let Ok(resolved) = self.require_resolved_unit_in(&path, Some(m.context)) else {
                    continue;
                };
                let locals = collect_local_elements(&resolved, name);
                cache.insert(path.clone(), (locals, ResolvedUnitRefClone::of(&resolved)));
            }
            let (locals, unit) = cache.get(&path).cloned().unwrap();
            let mut locals = locals;
            if is_field {
                let avoidable: Vec<LocalElement> = locals
                    .iter()
                    .filter(|l| l.can_avoid_shadow_for_field)
                    .cloned()
                    .collect();
                for local in &avoidable {
                    let s = SElem {
                        lib: unit.library.clone(),
                        unit: unit.index,
                        id: local.element,
                    };
                    for r in self.search_references(&s) {
                        if let Some(e) = self.match_element(&r)
                            && let Some(k) = e.identity()
                            && !unshadowed.contains(&k)
                        {
                            unshadowed.push(k);
                        }
                    }
                }
                locals.retain(|l| !l.can_avoid_shadow_for_field);
                if let Some(entry) = cache.get_mut(&path) {
                    entry.0.retain(|l| !l.can_avoid_shadow_for_field);
                }
            }
            for local in &locals {
                if let Some((start, end)) = local.visible_range
                    && start <= m.offset + m.length
                    && m.offset <= end
                {
                    return (
                        Some((kind_name(local.element), local.name.clone())),
                        unshadowed,
                    );
                }
            }
        }
        (None, unshadowed)
    }

    /// Dart `_ExtensionMemberValidator.validate`.
    fn check_extension_member(
        &mut self,
        rename: &mut Rename,
        extension: ElementId,
    ) -> RefactoringStatus {
        let mut result = RefactoringStatus::new();
        let Target::Element(element) = rename.target.clone() else {
            return result;
        };
        let name = rename.new_name.clone();
        element.with(|ctx| {
            for member in children_named(ctx, extension, &name) {
                result.add_error(
                    format_list(
                        "Extension '{0}' already declares {1} with name '{2}'.",
                        &[&display_name(ctx, extension), kind_name(member), &name],
                    ),
                    None,
                );
            }
        });
        let references = self.search_references(&element);
        let (conflict, _) = self.local_shadowing_element(&references, &name, false);
        if let Some((kind, local_name)) = conflict {
            result.add_error(
                format_list(
                    "Usage of renamed {0} will be shadowed by {1} '{2}'.",
                    &[kind_name(element.id), kind, &local_name],
                ),
                None,
            );
        }
        rename.extension_references = references;
        result
    }
}

/// A local element named like the new name, with its visible range
/// (Dart `_LocalElementsCollector` and `VisibleRangesComputer`).
#[derive(Clone)]
struct LocalElement {
    element: ElementId,
    name: String,
    visible_range: Option<(u32, u32)>,
    /// Dart `canAvoidShadowForField`: a local variable, top-level variable
    /// or parameter.
    can_avoid_shadow_for_field: bool,
}

/// Dart `_LocalElementsCollector(name)` over the unit, with the visible
/// ranges of `VisibleRangesComputer`.
fn collect_local_elements(resolved: &ResolvedUnitRef, name: &str) -> Vec<LocalElement> {
    let sink = NoopSink;
    let ctx = resolved.ctx(&sink);
    let unit = resolved.unit();
    let mut collector = LocalCollector {
        ctx: &ctx,
        tables: &unit.tables,
        name,
        out: Vec::new(),
    };
    unit.ast.accept(unit.unit, &mut collector);
    let ranges = visible_ranges(&ctx, &unit.ast, &unit.tables, unit.unit);
    collector
        .out
        .into_iter()
        .map(|e| LocalElement {
            element: e,
            name: display_name(&ctx, e),
            visible_range: ranges.get(&e).copied(),
            can_avoid_shadow_for_field: matches!(
                e.kind(),
                dartr_element::ElementKind::LocalVariable
                    | dartr_element::ElementKind::TopLevelVariable
                    | dartr_element::ElementKind::Parameter
            ),
        })
        .collect()
}

struct LocalCollector<'c, 'a> {
    ctx: &'c Ctx<'a>,
    tables: &'c dartr_element::ResolutionTables,
    name: &'c str,
    out: Vec<ElementId>,
}

impl AstVisitor for LocalCollector<'_, '_> {
    fn visit_node(&mut self, ast: &Ast, node: NodeId) {
        if let Some(p) = ast.cast::<FormalParameter>(node) {
            if formal_parameter_name(ast, p).is_some_and(|t| ast.tokens.lexeme(t) == self.name)
                && let Some(e) = support::declared_element(self.ctx, self.tables, node)
            {
                self.out.push(e);
            }
        } else if let Some(f) = ast.cast::<FunctionDeclaration>(node) {
            if ast.tokens.lexeme(ast[f].name) == self.name
                && let Some(e) = support::declared_element(self.ctx, self.tables, node)
                && e.tag() == Tag::LocalFunction
            {
                self.out.push(e);
            }
        } else if let Some(v) = ast.cast::<VariableDeclaration>(node) {
            if ast.tokens.lexeme(ast[v].name) == self.name
                && let Some(e) = support::declared_element(self.ctx, self.tables, node)
                && e.tag() == Tag::LocalVariable
            {
                self.out.push(e);
            }
        } else if let Some(s) = ast.cast::<SimpleIdentifier>(node) {
            // A top-level variable through its getter or setter.
            let mut element = support::element_of(self.ctx, self.tables, s);
            if let Some(parent) = ast.parent(s)
                && let Some(a) = ast.cast::<AssignmentExpression>(parent)
                && ast[a].left_hand_side.raw() == s.raw()
            {
                element = self
                    .tables
                    .write_element
                    .get(parent)
                    .map(|e| member::base_element(self.ctx, *e));
            }
            if let Some(e) = element
                && matches!(e.tag(), Tag::Getter | Tag::Setter)
                && ast.tokens.lexeme(ast[s].token) == self.name
                && let Some(v) =
                    dartr_resolver::element_metadata::accessor_variable_any(self.ctx, e)
                && v.tag() == Tag::TopLevelVariable
            {
                self.out.push(v);
            }
        }
        ast.visit_children(node, self);
    }
}

/// Dart `VisibleRangesComputer.forNode(unit)`: the visible range (start,
/// end) of each local variable, local function and parameter.
fn visible_ranges(
    ctx: &Ctx<'_>,
    ast: &Ast,
    tables: &dartr_element::ResolutionTables,
    root: Id<CompilationUnit>,
) -> HashMap<ElementId, (u32, u32)> {
    let mut v = VisibleRanges {
        ctx,
        tables,
        map: HashMap::new(),
    };
    ast.accept(root, &mut v);
    v.map
}

struct VisibleRanges<'c, 'a> {
    ctx: &'c Ctx<'a>,
    tables: &'c dartr_element::ResolutionTables,
    map: HashMap<ElementId, (u32, u32)>,
}

impl VisibleRanges<'_, '_> {
    fn add_local(&mut self, ast: &Ast, scope: NodeId, element: Option<ElementId>) {
        if let Some(e) = element
            && (is_local(e) || is_parameter(e))
        {
            self.map.insert(e, (ast.offset(scope), ast.end(scope)));
        }
    }
}

impl AstVisitor for VisibleRanges<'_, '_> {
    fn visit_node(&mut self, ast: &Ast, node: NodeId) {
        if let Some(c) = ast.cast::<CatchClause>(node) {
            if let Some(p) = ast[c].exception_parameter {
                let e = support::declared_element(self.ctx, self.tables, p);
                self.add_local(ast, node, e);
            }
            if let Some(p) = ast[c].stack_trace_parameter {
                let e = support::declared_element(self.ctx, self.tables, p);
                self.add_local(ast, node, e);
            }
            ast.accept(ast[c].body, self);
            return;
        }
        if ast.is::<FormalParameter>(node) {
            if let Some(e) = support::declared_element(self.ctx, self.tables, node)
                && is_parameter(e)
                && let Some(body) = function_body_of_parameter(ast, node)
            {
                self.map.insert(e, (ast.offset(body), ast.end(body)));
            }
            return;
        }
        if let Some(f) = ast.cast::<ForPartsWithDeclarations>(node) {
            let Some(lp) = ast.parent(node) else { return };
            for &v in ast.list(ast[ast[f].variables].variables) {
                let e = support::declared_element(self.ctx, self.tables, v);
                self.add_local(ast, lp, e);
                if let Some(i) = ast[v].initializer {
                    ast.accept(i, self);
                }
            }
            return;
        }
        if let Some(f) = ast.cast::<FunctionDeclaration>(node)
            && let Some(block) = ast.parent(node).and_then(|p| ast.parent(p))
            && ast.is::<Block>(block)
        {
            if let Some(e) = support::declared_element(self.ctx, self.tables, f) {
                self.map.insert(e, (ast.offset(block), ast.end(block)));
            }
        }
        if let Some(s) = ast.cast::<VariableDeclarationStatement>(node) {
            if let Some(block) = ast.parent(node) {
                for &v in ast.list(ast[ast[s].variables].variables) {
                    let e = support::declared_element(self.ctx, self.tables, v);
                    self.add_local(ast, block, e);
                    if let Some(i) = ast[v].initializer {
                        ast.accept(i, self);
                    }
                }
            }
            return;
        }
        ast.visit_children(node, self);
    }
}

/// Dart `VisibleRangesComputer._getFunctionBody`.
fn function_body_of_parameter(ast: &Ast, parameter: NodeId) -> Option<NodeId> {
    let parent = ast.parent(ast.parent(parameter)?)?;
    if let Some(c) = ast.cast::<ConstructorDeclaration>(parent) {
        return Some(ast[c].body.raw());
    }
    if let Some(f) = ast.cast::<FunctionExpression>(parent) {
        return Some(ast[f].body.raw());
    }
    if let Some(m) = ast.cast::<MethodDeclaration>(parent) {
        return Some(ast[m].body.raw());
    }
    None
}

// -------------------------------------------------------------------------
// Changes.

impl Engine<'_> {
    /// Dart `createChange`.
    pub fn create_change(&mut self, rename: &mut Rename) -> SourceChange {
        let refactoring_name = self.refactoring_name(rename);
        let message = format!(
            "{refactoring_name} '{}' to '{}'",
            rename.old_name, rename.new_name
        );
        let mut acc = ChangeAcc::default();
        let new_name = rename.new_name.clone();
        match rename.kind.clone() {
            Kind::Import => self.import_change(rename, &mut acc),
            Kind::Local => {
                let Target::Element(e) = rename.target.clone() else {
                    unreachable!()
                };
                self.local_change(&e, &new_name, &mut acc);
            }
            Kind::Label | Kind::Library | Kind::TypeParameter => {
                let Target::Element(e) = rename.target.clone() else {
                    unreachable!()
                };
                self.rename_element(&e, &new_name, &mut acc);
            }
            Kind::Parameter => self.parameter_change(rename, &mut acc),
            Kind::UnitMember => {
                let Target::Element(e) = rename.target.clone() else {
                    unreachable!()
                };
                let elements = e.with(|ctx| origin_getter_setter(ctx, e.id));
                for x in elements {
                    self.rename_element(&e.same(x), &new_name, &mut acc);
                }
                if let (Some(state), Some(state_name)) = (
                    rename.flutter_state.clone(),
                    Self::flutter_state_new_name(rename),
                ) {
                    self.rename_element(&state, &state_name, &mut acc);
                }
            }
            Kind::ClassMember { .. } => self.class_member_change(rename, &mut acc),
            Kind::ExtensionMember { .. } => {
                let Target::Element(e) = rename.target.clone() else {
                    unreachable!()
                };
                for x in e.with(|ctx| origin_getter_setter(ctx, e.id)) {
                    self.add_declaration_edit(&e.same(x), &new_name, &mut acc);
                }
                let references = rename.extension_references.clone();
                self.add_reference_edits(&references, &new_name, &mut acc);
            }
            Kind::Constructor => self.constructor_change(rename, &mut acc),
        }
        acc.into_change(message)
    }

    /// Dart `refactoringName`.
    fn refactoring_name(&self, rename: &Rename) -> &'static str {
        let tag = match &rename.target {
            Target::Element(e) => e.id.tag(),
            Target::Import { .. } => return "Rename Import Prefix",
        };
        match rename.kind {
            Kind::Import => "Rename Import Prefix",
            Kind::Local => {
                if tag == Tag::LocalFunction {
                    "Rename Local Function"
                } else {
                    "Rename Local Variable"
                }
            }
            Kind::Parameter => "Rename Parameter",
            Kind::Label => "Rename Label",
            Kind::Library => "Rename Library",
            Kind::TypeParameter => "Rename Type Parameter",
            Kind::Constructor => "Rename Constructor",
            Kind::UnitMember => match tag {
                Tag::Extension => "Rename Extension",
                Tag::ExtensionType => "Rename Extension Type",
                Tag::TopLevelFunction => "Rename Top-Level Function",
                Tag::TopLevelVariable => "Rename Top-Level Variable",
                Tag::TypeAlias => "Rename Type Alias",
                _ => "Rename Class",
            },
            Kind::ClassMember { .. } | Kind::ExtensionMember { .. } => match tag {
                Tag::TypeParameter => "Rename Type Parameter",
                Tag::Field => "Rename Field",
                _ => "Rename Method",
            },
        }
    }

    /// Dart `RenameProcessor2.renameElement`.
    fn rename_element(&mut self, element: &SElem, new_name: &str, acc: &mut ChangeAcc) {
        self.add_declaration_edit(element, new_name, acc);
        let matches = self.search_references(element);
        self.add_reference_edits(&matches, new_name, acc);
    }

    /// Dart `RenameProcessor2.addDeclarationEdit`.
    fn add_declaration_edit(&mut self, element: &SElem, new_name: &str, acc: &mut ChangeAcc) {
        if element.id.tag() == Tag::Library {
            // Dart `LibraryElementImpl.nameOffset` and `nameLength`: the name
            // of the library directive.
            let path = element.with(|ctx| element_path(ctx, element.id));
            let Some(path) = path else { return };
            let Ok(resolved) = self.require_resolved_unit_in(&path, Some(element.lib.context))
            else {
                return;
            };
            let unit = resolved.unit();
            let ast = &unit.ast;
            for &d in ast.list_raw(ast[unit.unit].directives) {
                if let Some(l) = ast.cast::<LibraryDirective>(d)
                    && let Some(name) = ast[l].name
                {
                    acc.add(&path, ast.offset(name), ast.length(name), new_name);
                }
            }
            return;
        }
        if !self.workspace_contains(&Target::Element(element.clone())) {
            return;
        }
        let supports_private_named = element
            .with(|ctx| support::library_of(ctx, element.id).is_some())
            && self.supports_private_named_parameters(element);
        let ranges = element.with(|ctx| {
            let mut out = Vec::new();
            for (path, offset, length, fragment) in fragment_name_ranges(ctx, element.id) {
                let mut replacement = new_name.to_string();
                let mut length = length;
                if supports_private_named && fragment.tag() == Tag::FieldFormalParameter {
                    if let Some(private) = ctx
                        .fragment_data(fragment)
                        .and_then(|_| private_name_of(ctx, fragment))
                    {
                        length = utf16_len(&private);
                    }
                } else if supports_private_named
                    && fragment.tag() == Tag::SuperFormalParameter
                    && parameter_is_named(ctx, element.id)
                    && new_name.starts_with('_')
                {
                    replacement =
                        corresponding_public_name(new_name).unwrap_or_else(|| new_name.to_string());
                }
                out.push((path, offset, length, replacement));
            }
            out
        });
        for (path, offset, length, replacement) in ranges {
            acc.add(&path, offset, length, &replacement);
        }
    }

    /// Dart `RenameProcessor2.addReferenceEdits`.
    fn add_reference_edits(&mut self, matches: &[Match], new_name: &str, acc: &mut ChangeAcc) {
        for reference in source_references(matches) {
            if !self.reference_in_workspace(&reference) {
                continue;
            }
            acc.add(
                &reference.path,
                reference.offset,
                reference.length,
                new_name,
            );
        }
    }

    /// `workspace.containsElement(reference.element)`.
    fn reference_in_workspace(&mut self, reference: &Match) -> bool {
        match self.match_element(reference) {
            Some(e) => self.workspace_contains(&Target::Element(e)),
            None => self.workspace_contains_file(&reference.path),
        }
    }

    /// Dart `RenameLocalRefactoringImpl.buildChange`.
    fn local_change(&mut self, element: &SElem, new_name: &str, acc: &mut ChangeAcc) {
        if matches!(
            element.id.tag(),
            Tag::PatternVariable | Tag::BindPatternVariable | Tag::JoinPatternVariable
        ) {
            let (declared, field_names) = element.with(|ctx| {
                let mut root = element.id;
                while let Some(j) = dartr_resolver::element_ext::pattern_variable_join(ctx, root) {
                    root = j;
                }
                let declared: Vec<ElementId> = if root.tag() == Tag::JoinPatternVariable {
                    let mut out = Vec::new();
                    let mut stack = vec![root];
                    while let Some(v) = stack.pop() {
                        if v.tag() == Tag::JoinPatternVariable {
                            let mut c =
                                dartr_resolver::element_ext::join_pattern_variable_components(
                                    ctx, v,
                                );
                            c.reverse();
                            stack.extend(c);
                        } else {
                            out.push(v);
                        }
                    }
                    out.into_iter()
                        .filter(|v| v.tag() == Tag::BindPatternVariable)
                        .collect()
                } else {
                    vec![element.id]
                };
                (declared, ())
            });
            let _ = field_names;
            for d in declared {
                self.add_declaration_edit(&element.same(d), new_name, acc);
                // `fieldNameWithImplicitName`: `:name` gets the old name.
                if let Some(colon) = self.implicit_field_name_colon(&element.same(d)) {
                    let old = element.with(|ctx| display_name(ctx, element.id));
                    let path = element
                        .with(|ctx| element_path(ctx, element.id))
                        .unwrap_or_default();
                    acc.add(&path, colon, 0, &old);
                }
            }
        } else {
            self.add_declaration_edit(element, new_name, acc);
        }
        let references = self.search_references(element);
        self.add_reference_edits(&references, new_name, acc);
    }

    /// The offset of the colon of the pattern field of the bind pattern
    /// variable [e] when the field name is implicit (`:name`).
    fn implicit_field_name_colon(&mut self, e: &SElem) -> Option<u32> {
        let path = e.with(|ctx| element_path(ctx, e.id))?;
        let resolved = self
            .require_resolved_unit_in(&path, Some(e.lib.context))
            .ok()?;
        let unit = resolved.unit();
        let ast = &unit.ast;
        let offset = e.with(|ctx| {
            let first = ctx.element_data(e.id)?.first_fragment;
            ctx.fragment_data(first)?.name_offset
        })?;
        let mut node = ast.node_covering(unit.unit, offset, 0);
        while let Some(n) = node {
            if let Some(f) = ast.cast::<PatternField>(n) {
                let name = ast[f].name?;
                if ast[name].name.is_none() {
                    return Some(ast.tokens.get(ast[name].colon).offset);
                }
                return None;
            }
            if ast.is::<Statement>(n) {
                return None;
            }
            node = ast.parent(n);
        }
        None
    }

    /// Dart `RenameParameterRefactoringImpl.buildChange`.
    fn parameter_change(&mut self, rename: &Rename, acc: &mut ChangeAcc) {
        let Target::Element(this) = rename.target.clone() else {
            return;
        };
        let new_name = rename.new_name.clone();
        for element in rename.parameters.clone() {
            let positional = element.with(|ctx| !parameter_is_named(ctx, element.id));
            let is_this = element.identity() == this.identity() && element.id == this.id
                || (element.lib.context == this.lib.context && element.id == this.id);
            if !is_this && positional && !rename.rename_all_positional {
                continue;
            }
            let mut field_renamed = false;
            if element.id.tag() == Tag::FieldFormalParameter {
                let field = element.with(|ctx| match ctx.any(element.id) {
                    dartr_element::AnyElement::FormalParameter(p) => p.field.get().map(|f| f.raw()),
                    _ => None,
                });
                if let Some(f) = field {
                    self.rename_element(&element.same(f), &new_name, acc);
                    field_renamed = true;
                }
            }
            if !field_renamed {
                self.add_declaration_edit(&element, &new_name, acc);
            }
            let mut references = self.search_references(&element);
            let mut kept = Vec::new();
            for r in references.drain(..) {
                let is_super = self
                    .match_element(&r)
                    .is_some_and(|e| e.id.tag() == Tag::SuperFormalParameter);
                if !is_super {
                    kept.push(r);
                }
            }
            self.add_reference_edits(&kept, &new_name, acc);
        }
    }

    /// Dart `RenameClassMemberRefactoringImpl.fillChange` (with
    /// `includePotential` false).
    fn class_member_change(&mut self, rename: &Rename, acc: &mut ChangeAcc) {
        let new_name = rename.new_name.clone();
        for e in rename.class_elements.clone() {
            let accessors = e.with(|ctx| origin_getter_setter(ctx, e.id));
            for x in accessors {
                self.add_declaration_edit(&e.same(x), &new_name, acc);
            }
        }
        // `_updateReferences`.
        let references = source_references(&rename.class_references);
        for reference in references {
            let Some(element) = self.match_element(&reference) else {
                continue;
            };
            if !self.workspace_contains(&Target::Element(element.clone())) {
                continue;
            }
            if new_name.starts_with('_')
                && element.id.tag() == Tag::FieldFormalParameter
                && element.with(|ctx| parameter_is_named(ctx, element.id))
                && !self.supports_private_named_parameters(&element)
            {
                self.private_named_formal_parameter_edit(&reference, &element, &new_name, acc);
                continue;
            }
            if is_parameter(element.id) {
                continue;
            }
            if !reference.is_qualified
                && element
                    .identity()
                    .is_some_and(|k| rename.class_unshadowed.contains(&k))
                && crate::element_locator::is_executable(element.id)
            {
                acc.add(
                    &reference.path,
                    reference.offset,
                    reference.length,
                    &format!("this.{new_name}"),
                );
                continue;
            }
            if reference.kind == RelationKind::IsReferencedByNamedArgument
                && new_name.starts_with('_')
            {
                let public =
                    corresponding_public_name(&new_name).unwrap_or_else(|| new_name.clone());
                acc.add(&reference.path, reference.offset, reference.length, &public);
            } else {
                acc.add(
                    &reference.path,
                    reference.offset,
                    reference.length,
                    &new_name,
                );
            }
        }
    }

    /// Dart `_addPrivateNamedFormalParameterEdit`.
    fn private_named_formal_parameter_edit(
        &mut self,
        reference: &Match,
        element: &SElem,
        new_name: &str,
        acc: &mut ChangeAcc,
    ) {
        let Some(path) = element.with(|ctx| element_path(ctx, element.id)) else {
            return;
        };
        let Ok(resolved) = self.require_resolved_unit_in(&path, Some(element.lib.context)) else {
            return;
        };
        let unit = resolved.unit();
        let ast = &unit.ast;
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let offset = element.with(|ctx| {
            let first = ctx.element_data(element.id)?.first_fragment;
            ctx.fragment_data(first)?.name_offset
        });
        let Some(offset) = offset else { return };
        let mut node = ast.node_covering(unit.unit, offset, 0);
        while let Some(n) = node {
            if let Some(p) = ast.cast::<FieldFormalParameter>(n) {
                let start = ast.tokens.get(ast[p].this_keyword).offset;
                let period_end = ast.tokens.get(ast[p].period).end();
                let type_name = element.with(|c| {
                    crate::completion::lsp::type_display(
                        c,
                        member::type_(c, ElemRef::Base(element.id)),
                    )
                });
                acc.add(
                    &reference.path,
                    start,
                    period_end - start,
                    &format!("{type_name} "),
                );
                let mut c = ast.parent(n);
                while let Some(cn) = c {
                    if let Some(constructor) = ast.cast::<ConstructorDeclaration>(cn) {
                        let previous_end = ast[constructor]
                            .separator
                            .map(|s| ast.tokens.get(s).end())
                            .unwrap_or_else(|| ast.end(ast[constructor].parameters));
                        let parameter_name = ast.tokens.lexeme(ast[p].name);
                        let mut replacement = format!("{new_name} = {parameter_name}");
                        replacement = if ast.list_raw(ast[constructor].initializers).is_empty() {
                            format!(" : {replacement}")
                        } else {
                            format!(" {replacement},")
                        };
                        acc.add(&reference.path, previous_end, 0, &replacement);
                        break;
                    }
                    c = ast.parent(cn);
                }
                let _ = &ctx;
                return;
            }
            node = ast.parent(n);
        }
    }

    /// Dart `RenameImportRefactoringImpl.buildChange`.
    fn import_change(&mut self, rename: &Rename, acc: &mut ChangeAcc) {
        let Target::Import { unit, index } = &rename.target else {
            return;
        };
        let resolved = unit.unit_ref();
        let path = unit.path();
        let new_name = rename.new_name.as_str();
        {
            let u = resolved.unit();
            let ast = &u.ast;
            let Some(node) = nth_import(ast, u.unit, *index) else {
                return;
            };
            let prefix = ast[node].prefix;
            let previous_end = ast[node]
                .deferred_keyword
                .map(|t| ast.tokens.get(t).end())
                .unwrap_or_else(|| ast.end(ast[node].uri));
            if new_name.is_empty() {
                if let Some(p) = prefix {
                    acc.add(&path, previous_end, ast.end(p) - previous_end, "");
                }
            } else {
                match prefix {
                    None => acc.add(&path, previous_end, 0, &format!(" as {new_name}")),
                    Some(p) => acc.add(&path, ast.offset(p), ast.length(p), new_name),
                }
            }
        }
        // `searchLibraryImportReferences`.
        for (file, offset, length, interpolation) in self.import_references(unit, *index) {
            if new_name.is_empty() {
                acc.add(&file, offset, length, "");
            } else if let Some((o, l, name)) = interpolation {
                acc.add(&file, o, l, &format!("{{{new_name}.{name}}}"));
            } else {
                acc.add(&file, offset, length, &format!("{new_name}."));
            }
        }
    }

    /// Dart `referencesLibraryImport` (`ImportElementReferencesVisitor`) in
    /// the files of the library of the import: (file, offset, length, and
    /// the identifier of an interpolation without braces).
    #[allow(clippy::type_complexity)]
    fn import_references(
        &mut self,
        unit: &ResolvedUnitRefClone,
        index: usize,
    ) -> Vec<(String, u32, u32, Option<(u32, u32, String)>)> {
        let mut out = Vec::new();
        if !self.workspace_contains_file(&unit.path()) {
            return out;
        }
        let library = unit.library.clone();
        let (imported, prefix_element) = {
            let sink = NoopSink;
            let ctx = library.ctx(unit.index, &sink);
            let first = ctx.get(library.library.library).first_fragment();
            let import = &ctx.fragment(first).library_imports[index];
            let names: Vec<ElementId> =
                crate::completion::declaration::import_namespace(&ctx, import)
                    .map(|(_, n)| n.into_iter().map(|(_, e)| e).collect())
                    .unwrap_or_default();
            let prefix = import
                .prefix
                .and_then(|p| ctx.fragment(p).element.try_get().copied());
            (names, prefix)
        };
        for i in 0..library.inputs.len() {
            let sink = NoopSink;
            let ctx = library.ctx(i, &sink);
            let u = &library.library.units[i];
            let path = library.inputs[i].path.to_string();
            let mut v = ImportReferences {
                ctx: &ctx,
                tables: &u.tables,
                imported: &imported,
                prefix: prefix_element,
                root: u.unit,
                out: Vec::new(),
            };
            u.ast.accept(u.unit, &mut v);
            for (offset, length, interpolation) in v.out {
                out.push((path.clone(), offset, length, interpolation));
            }
        }
        out
    }

    /// Dart `RenameConstructorRefactoringImpl.buildChange`.
    fn constructor_change(&mut self, rename: &Rename, acc: &mut ChangeAcc) {
        let Target::Element(element) = rename.target.clone() else {
            return;
        };
        let new_name = rename.new_name.clone();
        let matches = self.search_references(&element);
        for reference in source_references(&matches) {
            let Ok(resolved) =
                self.require_resolved_unit_in(&reference.path, Some(reference.context))
            else {
                continue;
            };
            let unit = resolved.unit();
            let ast = &unit.ast;
            let covering = ast.node_covering(unit.unit, reference.offset, 0);
            let covering_parent = covering.and_then(|c| ast.parent(c));
            let eol = end_of_line(ast);
            if let Some(class) = covering_parent.and_then(|p| ast.cast::<ClassDeclaration>(p)) {
                let class_name = ast
                    .tokens
                    .lexeme(support::class_name_token(ast, ast[class].name_part))
                    .to_string();
                let body = ast[class].body.raw();
                if let Some(b) = ast.cast::<BlockClassBody>(body) {
                    acc.add(
                        &reference.path,
                        ast.tokens.get(ast[b].left_bracket).end(),
                        0,
                        &format!("{eol}  {class_name}() : super.{new_name}();"),
                    );
                } else if let Some(b) = ast.cast::<EmptyClassBody>(body) {
                    let s = ast.tokens.get(ast[b].semicolon);
                    acc.add(
                        &reference.path,
                        s.offset,
                        s.end() - s.offset,
                        &format!(" {{{eol}  {class_name}() : super.{new_name}();{eol}}}"),
                    );
                }
                continue;
            }
            if let Some(c) = covering_parent.and_then(|p| ast.cast::<ConstructorDeclaration>(p))
                && ast[c]
                    .type_name
                    .is_some_and(|t| ast.offset(t) == reference.offset)
            {
                let initializers = ast.list_raw(ast[c].initializers);
                match initializers.last() {
                    Some(last) => acc.add(
                        &reference.path,
                        ast.end(*last),
                        0,
                        &format!(", super.{new_name}()"),
                    ),
                    None => acc.add(
                        &reference.path,
                        ast.end(ast[c].parameters),
                        0,
                        &format!(" : super.{new_name}()"),
                    ),
                }
                continue;
            }
            let dot_shorthand = matches!(
                reference.kind,
                RelationKind::IsInvokedByDotShorthandsConstructor
                    | RelationKind::IsReferencedByDotShorthandConstructorTearOff
            );
            let mut replacement = if !new_name.is_empty() {
                if dot_shorthand {
                    new_name.clone()
                } else {
                    format!(".{new_name}")
                }
            } else if dot_shorthand {
                "new".to_string()
            } else if reference.kind == RelationKind::IsReferencedByConstructorTearOff {
                ".new".to_string()
            } else {
                String::new()
            };
            if reference.kind == RelationKind::IsInvokedByEnumConstantWithoutArguments {
                replacement.push_str("()");
            }
            acc.add(
                &reference.path,
                reference.offset,
                reference.length,
                &replacement,
            );
        }
        let declaration = element.with(|ctx| {
            let flags = dartr_resolver::element_ext::first_fragment_flags(ctx, element.id);
            let first = ctx.element_data(element.id)?.first_fragment;
            let c = first.cast::<dartr_element::ConstructorFragment>()?;
            let f = ctx.fragment(c);
            let data = ctx.fragment_data(first)?;
            let path = crate::navigation::fragment_path(ctx, first)?;
            let name_len = data.name.map(|n| utf16_len(ctx.name_str(n)));
            Some((
                path,
                flags.contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_ORIGIN_IMPLICIT_DEFAULT),
                f.type_name_offset,
                f.type_name.map(|n| utf16_len(ctx.name_str(n))),
                f.period_offset,
                data.name_offset,
                name_len,
                f.new_keyword_offset,
                f.factory_keyword_offset,
            ))
        });
        let Some((
            path,
            implicit,
            type_name_offset,
            type_name_len,
            period,
            name_offset,
            name_len,
            new_kw,
            factory_kw,
        )) = declaration
        else {
            return;
        };
        if implicit {
            self.replace_synthetic_constructor(&element, &new_name, acc);
        } else if let Some(type_offset) = type_name_offset {
            match period {
                Some(p) => {
                    let end = name_offset.unwrap_or(p) + name_len.unwrap_or(0);
                    acc.add(
                        &path,
                        p,
                        end - p,
                        &if new_name.is_empty() {
                            String::new()
                        } else {
                            format!(".{new_name}")
                        },
                    );
                }
                None => acc.add(
                    &path,
                    type_offset + type_name_len.unwrap_or(0),
                    0,
                    &if new_name.is_empty() {
                        String::new()
                    } else {
                        format!(".{new_name}")
                    },
                ),
            }
        } else {
            let offset = match (new_kw, factory_kw) {
                (Some(n), _) => n + 3,
                (None, Some(f)) => f + 7,
                _ => return,
            };
            let end = match name_offset {
                Some(o) => o + name_len.unwrap_or(0),
                None => offset,
            };
            acc.add(
                &path,
                offset,
                end - offset,
                &if new_name.is_empty() {
                    String::new()
                } else {
                    format!(" {new_name}")
                },
            );
        }
    }

    /// Dart `_replaceSynthetic`: inserts the constructor in the class
    /// (Dart `insertConstructor` + `writeConstructorDeclaration`).
    fn replace_synthetic_constructor(
        &mut self,
        element: &SElem,
        new_name: &str,
        acc: &mut ChangeAcc,
    ) {
        let Some(class) = element.with(|ctx| enclosing(ctx, element.id)) else {
            return;
        };
        let Some(path) = element.with(|ctx| element_path(ctx, class)) else {
            return;
        };
        let class_name = element.with(|ctx| ctx.element_name(class).unwrap_or("").to_string());
        let Ok(resolved) = self.require_resolved_unit_in(&path, Some(element.lib.context)) else {
            return;
        };
        let unit = resolved.unit();
        let ast = &unit.ast;
        let offset = element.with(|ctx| {
            let first = ctx.element_data(class)?.first_fragment;
            ctx.fragment_data(first)?.name_offset
        });
        let Some(offset) = offset else { return };
        let mut node = ast.node_covering(unit.unit, offset, 0);
        while let Some(n) = node {
            if ast.is::<ClassDeclaration>(n) || ast.is::<EnumDeclaration>(n) {
                break;
            }
            node = ast.parent(n);
        }
        let Some(declaration) = node else { return };
        let is_enum = ast.is::<EnumDeclaration>(declaration);
        let mut text = String::new();
        if is_enum {
            text.push_str("const ");
        }
        text.push_str(&class_name);
        if !new_name.is_empty() {
            text.push('.');
            text.push_str(new_name);
        }
        text.push_str("();");
        let eol = end_of_line(ast).to_string();
        let mut change = None;
        self.host.with_change_workspace(&mut |workspace| {
            let mut builder =
                crate::correction::change_builder::ChangeBuilder::new(workspace, Some(&eol));
            builder.add_dart_file_edit(&path, |b| {
                b.insert_constructor(declaration, !new_name.is_empty(), |eb| eb.write(&text));
            });
            change = Some(builder.source_change());
        });
        let Some(change) = change else { return };
        for file in change.edits {
            for edit in file.edits.iter().rev() {
                acc.add(&file.file, edit.offset, edit.length, &edit.replacement);
            }
        }
    }
}

/// Dart `PropertyInducingElement.isOriginGetterSetter` elements: the
/// getter and setter of a variable that a getter or setter declares; else
/// the element itself.
fn origin_getter_setter(ctx: &Ctx<'_>, e: ElementId) -> Vec<ElementId> {
    if matches!(e.tag(), Tag::Field | Tag::TopLevelVariable) {
        let flags = dartr_resolver::element_ext::first_fragment_flags(ctx, e);
        if flags.contains(FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_GETTER_SETTER) {
            let (getter, setter) = match ctx.any(e) {
                dartr_element::AnyElement::Field(f) => {
                    (f.getter.map(|g| g.raw()), f.setter.map(|s| s.raw()))
                }
                dartr_element::AnyElement::TopLevelVariable(v) => {
                    (v.getter.map(|g| g.raw()), v.setter.map(|s| s.raw()))
                }
                _ => (None, None),
            };
            return getter.into_iter().chain(setter).collect();
        }
    }
    vec![e]
}

/// The private name of a field formal parameter fragment (Dart
/// `FieldFormalParameterFragment.privateName`).
fn private_name_of(ctx: &Ctx<'_>, fragment: dartr_element::FragmentId) -> Option<String> {
    let f = fragment.cast::<dartr_element::FormalParameterFragment>()?;
    ctx.fragment(f)
        .private_name
        .map(|n| ctx.name_str(n).to_string())
}

/// Dart `correspondingPublicName`.
fn corresponding_public_name(name: &str) -> Option<String> {
    let stripped = name.strip_prefix('_')?;
    let first = stripped.chars().next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    Some(stripped.to_string())
}

/// Dart `CorrectionUtils.endOfLine` of the unit.
fn end_of_line(ast: &Ast) -> &'static str {
    let source = &ast.tokens.source;
    match source.find('\n') {
        Some(i) if i > 0 && source.as_bytes()[i - 1] == b'\r' => "\r\n",
        _ => "\n",
    }
}

/// Dart `ImportElementReferencesVisitor`.
struct ImportReferences<'c, 'a> {
    ctx: &'c Ctx<'a>,
    tables: &'c dartr_element::ResolutionTables,
    imported: &'c [ElementId],
    prefix: Option<ElementId>,
    root: Id<CompilationUnit>,
    #[allow(clippy::type_complexity)]
    out: Vec<(u32, u32, Option<(u32, u32, String)>)>,
}

impl ImportReferences<'_, '_> {
    fn contains(&self, e: Option<ElementId>) -> bool {
        e.is_some_and(|e| self.imported.contains(&e))
    }

    fn add(&mut self, ast: &Ast, offset: u32, length: u32) {
        // `_getInterpolationIdentifier`: an identifier in `$name` (no
        // braces).
        let mut interpolation = None;
        if let Some(n) = ast.node_covering(self.root, offset, 0)
            && let Some(s) = ast.cast::<SimpleIdentifier>(n)
            && let Some(parent) = ast.parent(n)
            && let Some(i) = ast.cast::<InterpolationExpression>(parent)
            && ast[i].right_bracket.is_none()
        {
            interpolation = Some((
                ast.offset(n),
                ast.length(n),
                ast.tokens.lexeme(ast[s].token).to_string(),
            ));
        }
        self.out.push((offset, length, interpolation));
    }
}

impl AstVisitor for ImportReferences<'_, '_> {
    fn visit_export_directive(&mut self, _ast: &Ast, _node: Id<ExportDirective>) {}

    fn visit_import_directive(&mut self, _ast: &Ast, _node: Id<ImportDirective>) {}

    fn visit_named_type(&mut self, ast: &Ast, node: Id<NamedType>) {
        let element = support::element_of(self.ctx, self.tables, node);
        if self.contains(element) {
            let import_prefix = ast[node].import_prefix;
            match self.prefix {
                None => {
                    if import_prefix.is_none() {
                        let o = ast.offset(node);
                        self.add(ast, o, 0);
                    }
                }
                Some(p) => {
                    if let Some(ip) = import_prefix
                        && support::element_of(self.ctx, self.tables, ip) == Some(p)
                    {
                        let o = ast.offset(ip);
                        let end = ast.tokens.get(ast[ip].period).end();
                        self.add(ast, o, end - o);
                    }
                }
            }
        }
        if let Some(p) = ast[node].import_prefix {
            ast.accept(p, self);
        }
        if let Some(t) = ast[node].type_arguments {
            ast.accept(t, self);
        }
    }

    fn visit_simple_identifier(&mut self, ast: &Ast, node: Id<SimpleIdentifier>) {
        if support::in_declaration_context(ast, node) {
            return;
        }
        match self.prefix {
            Some(prefix) => {
                if support::element_of(self.ctx, self.tables, node) != Some(prefix) {
                    return;
                }
                let Some(parent) = ast.parent(node) else {
                    return;
                };
                if let Some(p) = ast.cast::<PrefixedIdentifier>(parent)
                    && ast[p].prefix == node
                {
                    let element = support::write_or_read_element(
                        self.ctx,
                        ast,
                        self.tables,
                        ast[p].identifier,
                    )
                    .or_else(|| support::element_of(self.ctx, self.tables, ast[p].identifier));
                    if self.contains(element) {
                        let o = ast.offset(node);
                        let next = ast.offset(ast[p].identifier);
                        self.add(ast, o, next - o);
                    }
                }
                if let Some(m) = ast.cast::<MethodInvocation>(parent)
                    && ast[m].target.map(|t| t.raw()) == Some(node.raw())
                {
                    let element = support::element_of(self.ctx, self.tables, ast[m].method_name);
                    if self.contains(element) {
                        let o = ast.offset(node);
                        let next = ast.offset(ast[m].method_name);
                        self.add(ast, o, next - o);
                    }
                }
            }
            None => {
                let element = support::write_or_read_element(self.ctx, ast, self.tables, node)
                    .or_else(|| support::element_of(self.ctx, self.tables, node));
                if self.contains(element) {
                    let o = ast.offset(node);
                    self.add(ast, o, 0);
                }
            }
        }
    }
}

// -------------------------------------------------------------------------
// Conflicts of locals and parameters.

impl Engine<'_> {
    /// Dart `ConflictValidatorVisitor` over the unit of [target].
    fn conflict_validate(
        &mut self,
        target: &SElem,
        new_name: &str,
        result: &mut RefactoringStatus,
    ) {
        let Some(path) = target.with(|ctx| element_path(ctx, target.id)) else {
            return;
        };
        let Ok(resolved) = self.require_resolved_unit_in(&path, Some(target.lib.context)) else {
            return;
        };
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let unit = resolved.unit();
        let ast = &unit.ast;
        let ranges = visible_ranges(&ctx, ast, &unit.tables, unit.unit);
        // The element in this resolution of the unit.
        let target_id = if std::sync::Arc::ptr_eq(&resolved.library, &target.lib) {
            target.id
        } else {
            return;
        };
        let target_range = ranges.get(&target_id).copied();
        let target_kind = kind_name(target_id);
        let mut v = ConflictVisitor {
            engine: self,
            ctx: &ctx,
            tables: &unit.tables,
            new_name,
            target_range,
            target_kind,
            ranges: &ranges,
            conflicting: Vec::new(),
            result,
        };
        ast.accept(unit.unit, &mut v);
    }
}

struct ConflictVisitor<'c, 'a, 'h> {
    engine: &'c Engine<'h>,
    ctx: &'c Ctx<'a>,
    tables: &'c dartr_element::ResolutionTables,
    new_name: &'c str,
    target_range: Option<(u32, u32)>,
    target_kind: &'static str,
    ranges: &'c HashMap<ElementId, (u32, u32)>,
    conflicting: Vec<ElementId>,
    result: &'c mut RefactoringStatus,
}

impl ConflictVisitor<'_, '_, '_> {
    /// Dart `_checkDeclaration`.
    fn check_declaration(
        &mut self,
        ast: &Ast,
        declared: Option<ElementId>,
        name: dartr_syntax::TokenId,
    ) {
        let Some(declared) = declared else { return };
        if ast.tokens.lexeme(name) != self.new_name {
            return;
        }
        // `_isVisibleWithTarget`.
        let visible = (is_local(declared) || is_parameter(declared))
            && match (self.ranges.get(&declared), self.target_range) {
                (Some(&(s, e)), Some((ts, te))) => s < te && ts < e,
                _ => false,
            };
        if visible {
            self.conflicting.push(declared);
            self.result.add_error(
                format!(
                    "Duplicate {} of name '{}'{}.",
                    kind_name(declared),
                    self.new_name,
                    declaration_location(self.ctx, declared)
                ),
                None,
            );
        }
    }
}

/// Dart `Element.declarationLocation` (rename_local.dart).
fn declaration_location(ctx: &Ctx<'_>, e: ElementId) -> String {
    let source = element_path(ctx, e)
        .map(|p| p.rsplit('/').next().unwrap_or("").to_string())
        .unwrap_or_default();
    let executable = enclosing(ctx, e);
    let mut class_name = String::new();
    let mut executable_name = String::new();
    match executable {
        Some(x) if x.tag() == Tag::Method => {
            class_name = enclosing(ctx, x)
                .filter(|c| c.tag() == Tag::Class)
                .map(|c| display_name(ctx, c))
                .unwrap_or_default();
            if !class_name.is_empty() && !display_name(ctx, x).is_empty() {
                class_name.push('.');
            }
            executable_name = display_name(ctx, x);
        }
        Some(x) if x.tag() == Tag::TopLevelFunction => executable_name = display_name(ctx, x),
        _ => {}
    }
    if executable_name.is_empty() {
        return format!(" in '{source}'");
    }
    format!(" at {class_name}{executable_name} in '{source}'")
}

impl AstVisitor for ConflictVisitor<'_, '_, '_> {
    fn visit_function_declaration(&mut self, ast: &Ast, node: Id<FunctionDeclaration>) {
        let declared = support::declared_element(self.ctx, self.tables, node);
        self.check_declaration(ast, declared, ast[node].name);
        ast.visit_children(node, self);
    }

    fn visit_variable_declaration(&mut self, ast: &Ast, node: Id<VariableDeclaration>) {
        let declared = support::declared_element(self.ctx, self.tables, node);
        self.check_declaration(ast, declared, ast[node].name);
        ast.visit_children(node, self);
    }

    fn visit_simple_identifier(&mut self, ast: &Ast, node: Id<SimpleIdentifier>) {
        let Some(element) = support::element_of(self.ctx, self.tables, node) else {
            return;
        };
        if self.ctx.element_name(element) != Some(self.new_name) {
            return;
        }
        if self.conflicting.contains(&element) {
            return;
        }
        let offset = ast.offset(node);
        let Some((start, end)) = self.target_range else {
            return;
        };
        let qualified = simple_identifier_is_qualified(ast, node);
        if start <= offset && offset < end && !qualified {
            let mut e = element;
            if matches!(e.tag(), Tag::Getter | Tag::Setter) {
                let flags = dartr_resolver::element_ext::first_fragment_flags(self.ctx, e);
                if flags.contains(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE)
                    && let Some(v) =
                        dartr_resolver::element_metadata::accessor_variable_any(self.ctx, e)
                {
                    e = v;
                }
            }
            let source = element_path(self.ctx, e)
                .map(|p| p.rsplit('/').next().unwrap_or("").to_string())
                .unwrap_or_default();
            self.result.add_error(
                format!(
                    "Usage of {} \"{}\" declared in \"{}\" will be shadowed by renamed {}.",
                    kind_name(e),
                    self.engine.qualified_name(self.ctx, e),
                    source,
                    self.target_kind
                ),
                None,
            );
        }
    }
}

/// Dart `SimpleIdentifier.isQualified`.
fn simple_identifier_is_qualified(ast: &Ast, node: Id<SimpleIdentifier>) -> bool {
    let Some(parent) = ast.parent(node) else {
        return false;
    };
    if let Some(p) = ast.cast::<PrefixedIdentifier>(parent) {
        return ast[p].identifier == node;
    }
    if let Some(p) = ast.cast::<PropertyAccess>(parent) {
        return ast[p].property_name == node;
    }
    if let Some(p) = ast.cast::<ConstructorName>(parent) {
        return ast[p].name == Some(node);
    }
    if let Some(m) = ast.cast::<MethodInvocation>(parent) {
        return ast[m].method_name == node
            && dartr_resolver::ast_ext::method_invocation_real_target(ast, m).is_some();
    }
    false
}

// Dart source: pkg/analysis_server/lib/src/services/correction/dart/import_library.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/namespace.dart (getExportedElement)

//! Dart `ImportLibrary` (a multi producer): imports a library that exports
//! the unresolved name, adds the name to a `show` combinator, or uses the
//! prefix of an existing import.

use dartr_ast::*;
use dartr_element::{Ctx, EId, ElementId, LibraryElement, Tag};

use super::super::change_builder::{ChangeBuilder, ChangeWorkspace};
use super::super::code_style::CodeStyleOptions;
use super::super::fix_kind::FixKind;
use super::super::generated::fix_kinds as k;
use super::super::imports::{dart_compare, library_uri};
use super::super::producer::*;

/// Dart `_ImportKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportKind {
    ForExtension,
    ForExtensionMember,
    ForExtensionType,
    ForFunction,
    ForTopLevelVariable,
    ForType,
}

impl ImportKind {
    pub fn from_name(name: &str) -> Option<ImportKind> {
        Some(match name {
            "ImportLibrary.forExtension" => ImportKind::ForExtension,
            "ImportLibrary.forExtensionMember" => ImportKind::ForExtensionMember,
            "ImportLibrary.forExtensionType" => ImportKind::ForExtensionType,
            "ImportLibrary.forFunction" => ImportKind::ForFunction,
            "ImportLibrary.forTopLevelVariable" => ImportKind::ForTopLevelVariable,
            "ImportLibrary.forType" => ImportKind::ForType,
            _ => return None,
        })
    }
}

const TYPE_KINDS: &[&str] = &[
    "CLASS",
    "ENUM",
    "EXTENSION_TYPE",
    "FUNCTION_TYPE_ALIAS",
    "MIXIN",
    "TYPE_ALIAS",
];
const FUNCTION_KINDS: &[&str] = &["FUNCTION", "TOP_LEVEL_VARIABLE"];

/// Dart `Element.kind.name` of a top-level element.
pub fn element_kind(element: ElementId) -> &'static str {
    match element.tag() {
        Tag::Class => "CLASS",
        Tag::Enum => "ENUM",
        Tag::Mixin => "MIXIN",
        Tag::Extension => "EXTENSION",
        Tag::ExtensionType => "EXTENSION_TYPE",
        Tag::TypeAlias => "TYPE_ALIAS",
        Tag::TopLevelFunction => "FUNCTION",
        Tag::TopLevelVariable => "TOP_LEVEL_VARIABLE",
        Tag::Getter | Tag::Setter => "TOP_LEVEL_VARIABLE",
        _ => "OTHER",
    }
}

/// Dart `getExportedElement`: the element that [library] exports with
/// [name].
pub fn exported_element(
    ctx: &Ctx<'_>,
    library: EId<LibraryElement>,
    name: &str,
) -> Option<ElementId> {
    let namespace = ctx.get(library).export_namespace.try_get()?;
    namespace
        .defined_names
        .iter()
        .find(|(n, _)| ctx.name_str(**n) == name)
        .map(|(_, e)| *e)
}

/// Dart `_PrefixedName`.
#[derive(Clone, Debug)]
struct PrefixedName {
    prefix: Option<String>,
    name: String,
    kinds: &'static [&'static str],
    can_be_prefixed: bool,
}

fn identifier_name(c: &ProducerContext<'_>, id: Id<SimpleIdentifier>) -> String {
    c.lexeme(c.ast[id].token).to_string()
}

/// Dart `_namesForMethodInvocation`.
fn names_for_method_invocation(
    c: &ProducerContext<'_>,
    name: String,
    parent: Option<NodeId>,
    kinds: &'static [&'static str],
) -> Vec<PrefixedName> {
    let ast = c.ast;
    let mut name = name;
    let mut prefix: Option<String> = None;
    let mut names = Vec::new();
    let pn = |prefix: Option<String>, name: String| PrefixedName {
        prefix,
        name,
        kinds,
        can_be_prefixed: true,
    };
    if let Some(m) = parent.and_then(|p| ast.cast::<MethodInvocation>(p)) {
        if let Some(target) = ast[m].target {
            names.push(pn(None, name.clone()));
            if target.raw() == c.node {
                prefix = Some(name.clone());
                name = identifier_name(c, ast[m].method_name);
            } else if let Some(t) = ast.cast::<SimpleIdentifier>(target) {
                prefix = Some(identifier_name(c, t));
            }
        }
    } else if let Some(p) = parent.and_then(|p| ast.cast::<PrefixedIdentifier>(p)) {
        names.push(pn(None, name.clone()));
        if ast[p].identifier.raw() != c.node {
            prefix = Some(name.clone());
            name = identifier_name(c, ast[p].identifier);
        } else {
            prefix = Some(identifier_name(c, ast[p].prefix));
        }
    }
    names.push(pn(prefix, name));
    names
}

/// Dart `_allPossibleNames`.
fn all_possible_names(c: &ProducerContext<'_>, kind: ImportKind) -> Vec<PrefixedName> {
    let ast = c.ast;
    let node = c.node;
    match kind {
        ImportKind::ForExtension => match ast.cast::<SimpleIdentifier>(node) {
            Some(id) => names_for_method_invocation(
                c,
                identifier_name(c, id),
                ast.parent(node),
                &["EXTENSION"],
            ),
            None => Vec::new(),
        },
        // Dart: extensions that declare the member (not ported).
        ImportKind::ForExtensionMember => Vec::new(),
        ImportKind::ForExtensionType => match ast.cast::<SimpleIdentifier>(node) {
            Some(id) => vec![PrefixedName {
                prefix: None,
                name: identifier_name(c, id),
                kinds: &["EXTENSION_TYPE"],
                can_be_prefixed: true,
            }],
            None => Vec::new(),
        },
        ImportKind::ForFunction => match ast.cast::<SimpleIdentifier>(node) {
            Some(id) => names_for_method_invocation(
                c,
                identifier_name(c, id),
                ast.parent(node),
                FUNCTION_KINDS,
            ),
            None => Vec::new(),
        },
        ImportKind::ForTopLevelVariable => names_for_top_level_variable(c),
        ImportKind::ForType => names_for_type(c),
    }
}

/// Dart `_namesForTopLevelVariable`.
fn names_for_top_level_variable(c: &ProducerContext<'_>) -> Vec<PrefixedName> {
    const KINDS: &[&str] = &["TOP_LEVEL_VARIABLE"];
    let ast = c.ast;
    let mut target = c.node;
    if let Some(a) = ast.cast::<Annotation>(target) {
        if c.element_of(ast[a].name.raw()).is_none() {
            if ast[a].arguments.is_some() {
                return Vec::new();
            }
            target = ast[a].name.raw();
        }
    }
    let pn = |prefix: Option<String>, name: String| PrefixedName {
        prefix,
        name,
        kinds: KINDS,
        can_be_prefixed: true,
    };
    if let Some(p) = ast.cast::<PrefixedIdentifier>(target) {
        let prefix = identifier_name(c, ast[p].prefix);
        return vec![
            pn(Some(prefix.clone()), identifier_name(c, ast[p].identifier)),
            pn(None, prefix),
        ];
    }
    let mut prefix = None;
    if let Some(p) = ast
        .parent(target)
        .and_then(|p| ast.cast::<PrefixedIdentifier>(p))
    {
        if ast[p].prefix.raw() == target {
            target = ast[p].identifier.raw();
            prefix = Some(identifier_name(c, ast[p].prefix));
        }
    }
    if let Some(id) = ast.cast::<SimpleIdentifier>(target) {
        return vec![pn(prefix, identifier_name(c, id))];
    }
    Vec::new()
}

/// Dart `nameOfType`.
fn name_of_type(c: &ProducerContext<'_>, node: NodeId) -> Option<String> {
    let ast = c.ast;
    if let Some(t) = ast.cast::<NamedType>(node) {
        let parent_is_constructor_name = ast
            .parent(node)
            .is_some_and(|p| ast.is::<ConstructorName>(p));
        if parent_is_constructor_name {
            if let Some(prefix) = ast[t].import_prefix {
                return Some(c.lexeme(ast[prefix].name).to_string());
            }
        }
        return Some(c.lexeme(ast[t].name).to_string());
    }
    if let Some(p) = ast.cast::<PrefixedIdentifier>(node) {
        return Some(identifier_name(c, ast[p].prefix));
    }
    ast.cast::<SimpleIdentifier>(node)
        .map(|id| identifier_name(c, id))
}

/// Dart `_namesForType`.
fn names_for_type(c: &ProducerContext<'_>) -> Vec<PrefixedName> {
    let ast = c.ast;
    let mut target = c.node;
    if let Some(a) = ast.cast::<Annotation>(target) {
        if c.element_of(ast[a].name.raw()).is_none() {
            if ast[a].period.is_some() && ast[a].arguments.is_none() {
                return Vec::new();
            }
            target = ast[a].name.raw();
        }
    }
    if let Some(id) = ast.cast::<SimpleIdentifier>(target) {
        return names_for_method_invocation(
            c,
            identifier_name(c, id),
            ast.parent(target),
            TYPE_KINDS,
        );
    }
    let mut prefix = None;
    if let Some(t) = ast.cast::<NamedType>(target) {
        let parent_is_constructor_name = ast
            .parent(target)
            .is_some_and(|p| ast.is::<ConstructorName>(p));
        if !parent_is_constructor_name {
            prefix = ast[t]
                .import_prefix
                .map(|p| c.lexeme(ast[p].name).to_string());
        }
    }
    if let Some(type_name) = name_of_type(c, target) {
        let mut names = vec![PrefixedName {
            prefix,
            name: type_name,
            kinds: TYPE_KINDS,
            can_be_prefixed: true,
        }];
        if let Some(p) = ast.cast::<PrefixedIdentifier>(target) {
            names.push(PrefixedName {
                prefix: Some(identifier_name(c, ast[p].prefix)),
                name: identifier_name(c, ast[p].identifier),
                kinds: TYPE_KINDS,
                can_be_prefixed: true,
            });
        }
        return names;
    }
    Vec::new()
}

/// Dart `ImportLibrary.producers`.
pub fn import_library_producers(
    kind: ImportKind,
    c: &ProducerContext<'_>,
    workspace: &mut dyn ChangeWorkspace,
) -> Vec<Box<dyn CorrectionProducer>> {
    let names = all_possible_names(c, kind);
    let mut producers = Vec::new();
    for name in names {
        producers.extend(import_library_for_element(c, workspace, &name));
    }
    producers
}

/// Dart `_isLibSrcPath`.
fn is_lib_src_path(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    (0..parts.len().saturating_sub(2)).any(|i| parts[i] == "lib" && parts[i + 1] == "src")
}

/// The package name of a `package:` URI.
fn package_name(uri: &str) -> Option<&str> {
    uri.strip_prefix("package:")?.split('/').next()
}

/// Dart `_importLibraryForElement`.
fn import_library_for_element(
    c: &ProducerContext<'_>,
    workspace: &mut dyn ChangeWorkspace,
    pn: &PrefixedName,
) -> Vec<Box<dyn CorrectionProducer>> {
    let name = &pn.name;
    let prefix = pn.prefix.clone();
    if name.starts_with('_') {
        return Vec::new();
    }
    let ast = c.ast;
    let ctx = c.ctx;
    let mut producers: Vec<Box<dyn CorrectionProducer>> = Vec::new();
    let mut already_imported: Vec<String> = Vec::new();
    for &directive in ast.list_raw(ast[c.unit].directives) {
        let Some(import) = ast.cast::<ImportDirective>(directive) else {
            continue;
        };
        let Some(library) = c
            .locator()
            .directive_library(directive)
            .and_then(|l| l.cast::<LibraryElement>())
        else {
            continue;
        };
        let Some(mut element) = exported_element(ctx, library, name) else {
            continue;
        };
        if matches!(element.tag(), Tag::Getter | Tag::Setter) {
            if let Some(v) = dartr_resolver::element_metadata::accessor_variable_any(ctx, element) {
                element = v;
            }
        }
        if !pn.kinds.contains(&element_kind(element)) {
            continue;
        }
        let uri_text = dartr_resolver::element_metadata::string_value(ast, ast[import].uri.raw())
            .unwrap_or_default();
        let combinators = import_combinators(ctx, c, directive);
        let import_prefix = ast[import].prefix.map(|p| identifier_name(c, p));
        let combinator = if combinators.is_empty() {
            None
        } else {
            Some(ImportLibraryCombinator {
                library_name: uri_text.clone(),
                combinators: combinators.clone(),
                updated_names: vec![name.clone()],
                remove_prefix: import_prefix.is_none(),
                multiple: false,
            })
        };
        if pn.can_be_prefixed {
            if let Some(import_prefix) = import_prefix {
                producers.push(Box::new(ImportLibraryPrefix {
                    library_uri: library_uri(ctx, library),
                    import_prefix,
                    edit_combinator: combinator,
                    node_prefix: prefix.clone(),
                }));
                continue;
            }
        }
        if let Some(combinator) = combinator {
            already_imported.push(library_uri(ctx, library));
            producers.push(Box::new(combinator));
        }
    }
    let declarations = workspace.top_level_declarations(c.path, name);
    let unit_library_uri = library_uri(ctx, c.resolved.library.library.library);
    let style = CodeStyleOptions { options: c.options };
    for declaration in declarations {
        if !pn.kinds.contains(&declaration.kind) {
            continue;
        }
        if already_imported.contains(&declaration.library_uri) {
            continue;
        }
        if declaration.library_uri.ends_with(".template.dart") {
            continue;
        }
        let no_prefix = prefix.as_deref().is_none_or(str::is_empty);
        let (fix_kind, fix_kind_show) = if declaration.library_uri.starts_with("dart:") {
            if no_prefix {
                (&k::IMPORT_LIBRARY_SDK, &k::IMPORT_LIBRARY_SDK_SHOW)
            } else {
                (
                    &k::IMPORT_LIBRARY_SDK_PREFIXED,
                    &k::IMPORT_LIBRARY_SDK_PREFIXED_SHOW,
                )
            }
        } else if is_lib_src_path(&declaration.library_path) {
            if no_prefix {
                (
                    &k::IMPORT_LIBRARY_PROJECT3,
                    &k::IMPORT_LIBRARY_PROJECT3_SHOW,
                )
            } else {
                (
                    &k::IMPORT_LIBRARY_PROJECT3_PREFIXED,
                    &k::IMPORT_LIBRARY_PROJECT3_PREFIXED_SHOW,
                )
            }
        } else if !declaration.declared_in_library {
            if no_prefix {
                (
                    &k::IMPORT_LIBRARY_PROJECT2,
                    &k::IMPORT_LIBRARY_PROJECT2_SHOW,
                )
            } else {
                (
                    &k::IMPORT_LIBRARY_PROJECT2_PREFIXED,
                    &k::IMPORT_LIBRARY_PROJECT2_PREFIXED_SHOW,
                )
            }
        } else if no_prefix {
            (
                &k::IMPORT_LIBRARY_PROJECT1,
                &k::IMPORT_LIBRARY_PROJECT1_SHOW,
            )
        } else {
            (
                &k::IMPORT_LIBRARY_PROJECT1_PREFIXED,
                &k::IMPORT_LIBRARY_PROJECT1_PREFIXED_SHOW,
            )
        };
        // Dart `isSamePackageAs`.
        let include_relative = match (
            package_name(&declaration.library_uri),
            package_name(&unit_library_uri),
        ) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        };
        let library = declaration.library_uri.clone();
        let make = |relative: bool,
                    kind: &'static FixKind,
                    show: Option<String>|
         -> Box<dyn CorrectionProducer> {
            Box::new(ImportLibraryUri {
                fix_kind: kind,
                library: library.clone(),
                prefix: prefix.clone(),
                show,
                relative,
                uri_text: String::new(),
            })
        };
        if !include_relative {
            producers.push(make(false, fix_kind, None));
            producers.push(make(false, fix_kind_show, Some(name.clone())));
            continue;
        }
        let use_package = style.use_package_uris();
        let use_relative = style.use_relative_uris();
        if use_package || !use_relative {
            producers.push(make(false, fix_kind, None));
            producers.push(make(false, fix_kind_show, Some(name.clone())));
        }
        if use_relative || !use_package {
            producers.push(make(true, fix_kind, None));
            producers.push(make(true, fix_kind_show, Some(name.clone())));
        }
    }
    producers
}

/// The combinators of the import [directive] (Dart `import.combinators`):
/// show or hide, the names, and the offset and end.
fn import_combinators(
    ctx: &Ctx<'_>,
    c: &ProducerContext<'_>,
    directive: NodeId,
) -> Vec<(bool, Vec<String>, u32, u32)> {
    let ast = c.ast;
    let _ = ctx;
    let Some(import) = ast.cast::<ImportDirective>(directive) else {
        return Vec::new();
    };
    let names_of = |list: NodeList<SimpleIdentifier>| -> Vec<String> {
        ast.list(list)
            .iter()
            .map(|n| identifier_name(c, *n))
            .collect()
    };
    let mut out = Vec::new();
    for &combinator in ast.list_raw(ast[import].combinators) {
        if let Some(s) = ast.cast::<ShowCombinator>(combinator) {
            out.push((
                true,
                names_of(ast[s].shown_names),
                ast.offset(s),
                ast.end(s),
            ));
        } else if let Some(h) = ast.cast::<HideCombinator>(combinator) {
            out.push((
                false,
                names_of(ast[h].hidden_names),
                ast.offset(h),
                ast.end(h),
            ));
        }
    }
    out
}

/// Dart `_ImportAbsoluteLibrary` and `_ImportRelativeLibrary`.
pub struct ImportLibraryUri {
    fix_kind: &'static FixKind,
    library: String,
    prefix: Option<String>,
    show: Option<String>,
    relative: bool,
    uri_text: String,
}

impl CorrectionProducer for ImportLibraryUri {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(self.fix_kind)
    }

    fn fix_arguments(&self) -> Vec<String> {
        let mut args = vec![self.uri_text.clone()];
        if let Some(p) = self.prefix.as_ref().filter(|p| !p.is_empty()) {
            args.push(p.clone());
        }
        args
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let mut uri_text = String::new();
        builder.add_dart_file_edit(c.path, |b| {
            let use_show = self.show.is_some();
            uri_text = if self.relative {
                b.import_library_with_relative_uri(
                    &self.library,
                    self.prefix.as_deref(),
                    self.show.as_deref(),
                    use_show,
                )
            } else {
                b.import_library_with_absolute_uri(
                    &self.library,
                    self.prefix.as_deref(),
                    self.show.as_deref(),
                    use_show,
                )
            };
        });
        self.uri_text = uri_text;
    }
}

/// Dart `_ImportLibraryCombinator` and `_ImportLibraryCombinatorMultiple`.
#[derive(Clone)]
pub struct ImportLibraryCombinator {
    library_name: String,
    combinators: Vec<(bool, Vec<String>, u32, u32)>,
    updated_names: Vec<String>,
    remove_prefix: bool,
    multiple: bool,
}

impl CorrectionProducer for ImportLibraryCombinator {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(if self.multiple {
            &k::IMPORT_LIBRARY_COMBINATOR_MULTIPLE
        } else {
            &k::IMPORT_LIBRARY_COMBINATOR
        })
    }

    fn fix_arguments(&self) -> Vec<String> {
        if !self.multiple {
            return vec![self.updated_names[0].clone(), self.library_name.clone()];
        }
        let others = self.updated_names.len() - 1;
        vec![
            self.updated_names[0].clone(),
            others.to_string(),
            if others == 1 {
                String::new()
            } else {
                "s".into()
            },
            self.library_name.clone(),
        ]
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let sort = c.code_style().sort_combinators();
        let ast = c.ast;
        let library_path = c
            .resolved
            .library
            .library
            .units
            .first()
            .map(|u| u.path.to_string())
            .unwrap_or_else(|| c.path.to_string());
        for (is_show, names, offset, end) in &self.combinators {
            let mut combinator_names: indexmap::IndexSet<String> = names.iter().cloned().collect();
            if *is_show {
                combinator_names.extend(self.updated_names.iter().cloned());
            } else {
                for n in &self.updated_names {
                    combinator_names.shift_remove(n);
                }
            }
            let mut names: Vec<String> = combinator_names.into_iter().collect();
            if sort {
                names.sort_by(|a, b| dart_compare(a, b));
            }
            let code = if names.is_empty() {
                String::new()
            } else {
                format!(
                    " {} {}",
                    if *is_show { "show" } else { "hide" },
                    names.join(", ")
                )
            };
            let prefix_range = if self.remove_prefix {
                if let Some(t) = ast.cast::<NamedType>(c.node) {
                    ast[t].import_prefix.map(|p| (ast.offset(p), ast.length(p)))
                } else if let Some(p) = ast.cast::<PrefixedIdentifier>(c.node) {
                    Some((ast.offset(ast[p].prefix), ast.length(ast[p].prefix)))
                } else {
                    None
                }
            } else {
                None
            };
            let (offset, end) = (*offset, *end);
            builder.add_dart_file_edit(&library_path, |b| {
                b.add_simple_replacement(offset - 1, end - offset + 1, &code);
                if let Some((o, l)) = prefix_range {
                    b.add_deletion(o, l);
                }
            });
        }
    }
}

/// Dart `_ImportLibraryPrefix`.
pub struct ImportLibraryPrefix {
    library_uri: String,
    import_prefix: String,
    edit_combinator: Option<ImportLibraryCombinator>,
    node_prefix: Option<String>,
}

impl CorrectionProducer for ImportLibraryPrefix {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::IMPORT_LIBRARY_PREFIX)
    }

    fn fix_arguments(&self) -> Vec<String> {
        vec![self.library_uri.clone(), self.import_prefix.clone()]
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        let ast = c.ast;
        let mut target = c.node;
        if let Some(a) = ast.cast::<Annotation>(target) {
            target = ast[a].name.raw();
        }
        if let Some(combinator) = &mut self.edit_combinator {
            combinator.compute(c, builder);
        }
        let prefix_name = self.import_prefix.clone();
        match &self.node_prefix {
            None => {
                let offset = ast.offset(target);
                builder.add_dart_file_edit(c.path, |b| {
                    b.add_simple_insertion(offset, &format!("{prefix_name}."))
                });
            }
            Some(node_prefix) if *node_prefix != prefix_name => {
                let range = if let Some(t) = ast
                    .cast::<NamedType>(target)
                    .filter(|t| ast[*t].import_prefix.is_some())
                {
                    c.range().node(ast[t].import_prefix.unwrap())
                } else if let Some(p) = ast.cast::<PrefixedIdentifier>(target) {
                    c.range().start_start(ast[p].prefix, ast[p].identifier)
                } else if let Some(m) = ast
                    .parent(target)
                    .and_then(|p| ast.cast::<MethodInvocation>(p))
                {
                    let Some(t) = ast[m].target.and_then(|t| ast.cast::<SimpleIdentifier>(t))
                    else {
                        return;
                    };
                    if identifier_name(c, t) != *node_prefix {
                        return;
                    }
                    c.range().start_start(t, ast[m].method_name)
                } else {
                    return;
                };
                builder.add_dart_file_edit(c.path, |b| {
                    b.add_simple_replacement(range.offset, range.length, &format!("{prefix_name}."))
                });
            }
            _ => {}
        }
    }
}

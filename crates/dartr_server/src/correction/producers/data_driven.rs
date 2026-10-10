// Dart source: pkg/analysis_server/lib/src/services/correction/dart/data_driven.dart (DataDriven, DataDrivenFix)
// Dart source: pkg/analysis_server/lib/src/services/correction/fix/data_driven/transform_set_manager.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/fix/data_driven/transform_set_parser.dart (the element transforms)
// Dart source: pkg/analysis_server/lib/src/services/correction/fix/data_driven/element_matcher.dart (ElementMatcher, _MatcherBuilder)
// Dart source: pkg/analysis_server/lib/src/services/correction/fix/data_driven/element_kind.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/fix/data_driven/rename.dart

//! Data-driven fixes (`fix_data.yaml`, as `dart fix` applies them).
//!
//! Ported: the element transforms whose changes are all `rename` changes
//! (no `oneOf` selectors, no `library` transforms). Transforms with other
//! changes (`addParameter`, `removeParameter`, `renameParameter`,
//! `replacedBy`, `addTypeParameter`, `changeParameterType`) give no fix.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use dartr_ast::*;
use dartr_element::{Tag, TypeKind};
use dartr_project::yaml::{NodeKind, YamlNode};
use dartr_syntax::TokenId;
use dartr_typesystem::type_ext::TypeExt;

use super::super::change_builder::{ChangeBuilder, ChangeWorkspace};
use super::super::fix_kind::FixKind;
use super::super::generated::fix_kinds as k;
use super::super::imports::{existing_imports, library_uri};
use super::super::producer::*;

/// Dart `ElementKind` of `element_kind.dart`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Class,
    Constant,
    Constructor,
    Enum,
    Extension,
    ExtensionType,
    Field,
    Function,
    Getter,
    Library,
    Method,
    Mixin,
    Setter,
    Typedef,
    Variable,
}

impl Kind {
    fn from_key(key: &str) -> Option<Kind> {
        Some(match key {
            "class" => Kind::Class,
            "constant" => Kind::Constant,
            "constructor" => Kind::Constructor,
            "enum" => Kind::Enum,
            "extension" => Kind::Extension,
            "extensionType" => Kind::ExtensionType,
            "field" => Kind::Field,
            "function" => Kind::Function,
            "getter" => Kind::Getter,
            "method" => Kind::Method,
            "mixin" => Kind::Mixin,
            "setter" => Kind::Setter,
            "typedef" => Kind::Typedef,
            "variable" => Kind::Variable,
            _ => return None,
        })
    }

    /// Dart `_containerKeyMap`.
    fn container_keys(self) -> &'static [&'static str] {
        match self {
            Kind::Constructor => &["inClass", "inExtensionType"],
            Kind::Constant => &["inEnum"],
            Kind::Field | Kind::Getter | Kind::Method | Kind::Setter => {
                &["inClass", "inExtension", "inExtensionType", "inMixin"]
            }
            _ => &[],
        }
    }
}

/// Dart `ElementDescriptor`.
#[derive(Clone, Debug)]
struct ElementDescriptor {
    library_uris: Vec<String>,
    kind: Kind,
    components: Vec<String>,
}

/// Dart `Transform` (with rename changes only).
#[derive(Clone, Debug)]
pub struct Transform {
    title: String,
    bulk_apply: bool,
    element: ElementDescriptor,
    /// The new names of the `rename` changes.
    renames: Vec<String>,
}

fn map_value<'a>(node: &'a YamlNode, key: &str) -> Option<&'a YamlNode> {
    node.as_map()?
        .iter()
        .find(|(k, _)| k.string_value() == Some(key))
        .map(|(_, v)| v)
}

fn string_of(node: Option<&YamlNode>) -> Option<String> {
    let node = node?;
    match &node.kind {
        NodeKind::Scalar(s) => Some(s.to_dart_string()).filter(|_| node.string_value().is_some()),
        _ => None,
    }
}

/// Dart `TransformSetParser.parse` (element transforms with rename
/// changes).
fn parse_transforms(text: &str, package_name: Option<&str>) -> Vec<Transform> {
    let Ok(root) = dartr_project::yaml::load_yaml_node(text) else {
        return Vec::new();
    };
    let Some(transforms) = map_value(&root, "transforms").and_then(|t| t.as_list()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for t in transforms {
        let Some(title) = string_of(map_value(t, "title")) else {
            continue;
        };
        if map_value(t, "date").is_none() || map_value(t, "oneOf").is_some() {
            continue;
        }
        let bulk_apply = map_value(t, "bulkApply")
            .and_then(|b| b.scalar())
            .map(|s| s.to_dart_string() == "true")
            .unwrap_or(true);
        let Some(element) = map_value(t, "element") else {
            continue;
        };
        let Some(uris) = map_value(element, "uris").and_then(|u| u.as_list()) else {
            continue;
        };
        let library_uris: Vec<String> = uris
            .iter()
            .filter_map(|u| u.string_value())
            .map(|u| match package_name {
                Some(p) if !u.starts_with("dart:") && !u.starts_with("package:") => {
                    format!("package:{p}/{u}")
                }
                _ => u.to_string(),
            })
            .collect();
        let Some(entries) = element.as_map() else {
            continue;
        };
        let mut kind_and_name = None;
        for (key, value) in entries {
            if let Some(kind) = key.string_value().and_then(Kind::from_key) {
                kind_and_name = string_of(Some(value)).map(|n| (kind, n));
            }
        }
        let Some((kind, name)) = kind_and_name else {
            continue;
        };
        let mut components = vec![name];
        let container = kind
            .container_keys()
            .iter()
            .find_map(|key| string_of(map_value(element, key)));
        match container {
            Some(c) => components.push(c),
            None => {
                if matches!(
                    kind,
                    Kind::Constructor | Kind::Constant | Kind::Method | Kind::Field
                ) {
                    continue;
                }
            }
        }
        let Some(changes) = map_value(t, "changes").and_then(|c| c.as_list()) else {
            continue;
        };
        let mut renames = Vec::new();
        let mut supported = !changes.is_empty();
        for change in changes {
            match string_of(map_value(change, "kind")).as_deref() {
                Some("rename") => match string_of(map_value(change, "newName")) {
                    Some(n) => renames.push(n),
                    None => supported = false,
                },
                _ => supported = false,
            }
        }
        if !supported {
            continue;
        }
        out.push(Transform {
            title,
            bulk_apply,
            element: ElementDescriptor {
                library_uris,
                kind,
                components,
            },
            renames,
        });
    }
    out
}

/// The parsed transforms of the fix data files, by path and content.
fn transform_sets(workspace: &dyn ChangeWorkspace, path: &str) -> Vec<Arc<Vec<Transform>>> {
    static CACHE: OnceLock<Mutex<HashMap<(String, u64), Arc<Vec<Transform>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut out = Vec::new();
    for (file, package) in workspace.fix_data_files(path) {
        let Some(content) = workspace.content(&file) else {
            continue;
        };
        let hash = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            content.hash(&mut h);
            h.finish()
        };
        let key = (file.clone(), hash);
        let mut guard = cache.lock().unwrap();
        let set = guard
            .entry(key)
            .or_insert_with(|| Arc::new(parse_transforms(&content, package.as_deref())))
            .clone();
        out.push(set);
    }
    out
}

/// Dart `ElementMatcher`.
struct Matcher {
    components: Vec<String>,
    kinds: Vec<Kind>,
}

/// Dart `ElementMatcher.matches` (without the supertype check for nodes
/// with fewer components).
fn matches(m: &Matcher, imported: &[String], e: &ElementDescriptor) -> bool {
    let ec = &e.components;
    let nc = &m.components;
    if nc.len() == ec.len() {
        if nc != ec {
            return false;
        }
    } else if nc.len() < ec.len() {
        if nc.len() + 1 == ec.len() && ec[0].is_empty() {
            if ec[1..] != nc[..] {
                return false;
            }
        } else if ec[..nc.len()] != nc[..] {
            return false;
        }
    } else if ec[0] != nc[1] {
        return false;
    }
    if !m.kinds.is_empty() && !m.kinds.contains(&e.kind) {
        return false;
    }
    if e.kind == Kind::Library {
        return true;
    }
    imported.iter().any(|u| e.library_uris.contains(u))
}

/// Dart `_MatcherBuilder`.
struct MatcherBuilder<'c, 'a> {
    c: &'c ProducerContext<'a>,
    matchers: Vec<Matcher>,
}

const MEMBER_KINDS: &[Kind] = &[
    Kind::Constant,
    Kind::Field,
    Kind::Function,
    Kind::Getter,
    Kind::Method,
    Kind::Setter,
];

impl MatcherBuilder<'_, '_> {
    fn add(&mut self, components: Vec<String>, kinds: &[Kind]) {
        self.matchers.push(Matcher {
            components,
            kinds: kinds.to_vec(),
        });
    }

    fn name(&self, id: Id<SimpleIdentifier>) -> String {
        self.c.lexeme(self.c.ast[id].token).to_string()
    }

    fn interface_type_name(&self, node: NodeId) -> Option<(String, Vec<String>)> {
        let ctx = self.c.ctx;
        let ty = self.c.tables.static_type.get(node).copied()?;
        let TypeKind::Interface { element, .. } = *ctx.ty(ty) else {
            return None;
        };
        let name = ctx.element_name(element.raw())?.to_string();
        let supers = ctx
            .all_supertypes(ty)
            .into_iter()
            .filter_map(|t| match *ctx.ty(t) {
                TypeKind::Interface { element, .. } => {
                    ctx.element_name(element.raw()).map(str::to_string)
                }
                _ => None,
            })
            .collect();
        Some((name, supers))
    }

    fn is_prefix(&self, node: Option<NodeId>) -> bool {
        node.filter(|n| self.c.ast.is::<SimpleIdentifier>(*n))
            .and_then(|n| self.c.element_of(n))
            .is_some_and(|e| e.tag() == Tag::Prefix)
    }

    /// Dart `_componentsFromIdentifier`.
    fn components_from_identifier(&self, id: Id<SimpleIdentifier>) -> Vec<String> {
        let c = self.c;
        let ast = c.ast;
        let mut element = c.element_of(id.raw());
        if element.is_none() {
            if let Some(a) = ast
                .parent(id)
                .and_then(|p| ast.cast::<AssignmentExpression>(p))
            {
                if ast[a].left_hand_side.raw() == id.raw() {
                    element = c
                        .tables
                        .write_element
                        .get(a.raw())
                        .map(|e| dartr_typesystem::member::base_element(c.ctx, *e));
                }
            }
        }
        if let Some(e) = element {
            if let Some(enclosing) = c.ctx.element_data(e).and_then(|d| d.enclosing) {
                if matches!(
                    enclosing.tag(),
                    Tag::Class | Tag::Enum | Tag::Mixin | Tag::ExtensionType | Tag::Extension
                ) {
                    if let Some(n) = c.ctx.element_name(enclosing) {
                        return vec![self.name(id), n.to_string()];
                    }
                }
            }
        }
        vec![self.name(id)]
    }

    /// Dart `_nameOfTarget`.
    fn name_of_target(&self, target: Option<NodeId>) -> Option<String> {
        let c = self.c;
        let target = target?;
        let ty = c.tables.static_type.get(target).copied();
        if let Some(id) = c.ast.cast::<SimpleIdentifier>(target) {
            return match ty {
                Some(t) => match *c.ctx.ty(t) {
                    TypeKind::Interface { element, .. } => {
                        c.ctx.element_name(element.raw()).map(str::to_string)
                    }
                    TypeKind::Dynamic => Some(self.name(id)),
                    _ => None,
                },
                None => Some(self.name(id)),
            };
        }
        match *c.ctx.ty(ty?) {
            TypeKind::Interface { element, .. } => {
                c.ctx.element_name(element.raw()).map(str::to_string)
            }
            _ => None,
        }
    }

    fn constructor_name(&mut self, node: Id<ConstructorName>) {
        let ast = self.c.ast;
        let constructor = ast[node].name.map(|n| self.name(n)).unwrap_or_default();
        let type_name = self.c.lexeme(ast[ast[node].type_].name).to_string();
        self.add(vec![constructor, type_name.clone()], &[Kind::Constructor]);
        self.add(
            vec![type_name],
            &[
                Kind::Class,
                Kind::Enum,
                Kind::ExtensionType,
                Kind::Typedef,
                Kind::Mixin,
            ],
        );
    }

    fn method_invocation(&mut self, m: Id<MethodInvocation>) {
        let ast = self.c.ast;
        let method_name = ast[m].method_name;
        let target = super::create::real_target(ast, m);
        if let Some((name, supers)) = target.and_then(|t| self.interface_type_name(t)) {
            self.add(
                vec![self.name(method_name), name],
                &[Kind::Constructor, Kind::Method],
            );
            for s in supers {
                self.add(vec![self.name(method_name), s], &[Kind::Method]);
            }
            return;
        }
        if let Some(target_name) = self.name_of_target(target) {
            self.add(
                vec![self.name(method_name), target_name],
                &[Kind::Constructor, Kind::Method],
            );
        } else if target.is_some() {
            let components = self.components_from_identifier(method_name);
            self.add(components, &[Kind::Constructor, Kind::Getter, Kind::Method]);
        } else {
            let components = self.components_from_identifier(method_name);
            self.add(
                components,
                &[
                    Kind::Class,
                    Kind::Constructor,
                    Kind::Enum,
                    Kind::Extension,
                    Kind::ExtensionType,
                    Kind::Function,
                    Kind::Getter,
                    Kind::Method,
                    Kind::Mixin,
                    Kind::Typedef,
                ],
            );
        }
    }

    fn named_type(&mut self, node: Id<NamedType>) {
        let ast = self.c.ast;
        if let Some(cn) = ast
            .parent(node)
            .and_then(|p| ast.cast::<ConstructorName>(p))
        {
            return self.constructor_name(cn);
        }
        let name = self.c.lexeme(ast[node].name).to_string();
        self.add(
            vec![name],
            &[
                Kind::Class,
                Kind::Enum,
                Kind::ExtensionType,
                Kind::Mixin,
                Kind::Typedef,
            ],
        );
    }

    fn prefixed_identifier(&mut self, node: Id<PrefixedIdentifier>) {
        let c = self.c;
        let ast = c.ast;
        if let Some(t) = ast.parent(node).and_then(|p| ast.cast::<NamedType>(p)) {
            return self.named_type(t);
        }
        let identifier = self.name(ast[node].identifier);
        let prefix = ast[node].prefix;
        if self.is_prefix(Some(prefix.raw())) {
            self.add(
                vec![identifier.clone()],
                &[
                    Kind::Class,
                    Kind::Enum,
                    Kind::Extension,
                    Kind::ExtensionType,
                    Kind::Mixin,
                    Kind::Typedef,
                ],
            );
            self.add(
                vec![identifier.clone()],
                &[
                    Kind::Constructor,
                    Kind::Function,
                    Kind::Getter,
                    Kind::Setter,
                    Kind::Variable,
                ],
            );
        }
        if let Some((name, supers)) = self.interface_type_name(prefix.raw()) {
            for e in std::iter::once(name).chain(supers) {
                self.add(vec![identifier.clone(), e], MEMBER_KINDS);
            }
        }
        if let Some(container) = c.element_of(prefix.raw()) {
            if matches!(
                container.tag(),
                Tag::Class | Tag::Enum | Tag::Mixin | Tag::ExtensionType | Tag::Extension
            ) {
                if let Some(n) = c.ctx.element_name(container) {
                    self.add(vec![identifier, n.to_string()], MEMBER_KINDS);
                }
            }
        }
    }

    fn property_access(&mut self, node: Id<PropertyAccess>) {
        let ast = self.c.ast;
        let property = ast[node].property_name;
        let target = ast[node].target.map(|t| t.raw());
        let components = match self.name_of_target(target) {
            Some(t) => vec![self.name(property), t],
            None => self.components_from_identifier(property),
        };
        self.add(components, MEMBER_KINDS);
    }

    fn argument_list(&mut self, node: NodeId) {
        let c = self.c;
        let ast = c.ast;
        let Some(parent) = ast.parent(node) else {
            return;
        };
        if let Some(a) = ast.cast::<Annotation>(parent) {
            let constructor = ast[a]
                .constructor_name
                .map(|n| self.name(n))
                .unwrap_or_default();
            let name = c.utils.get_node_text(ast[a].name);
            self.add(vec![constructor, name], &[Kind::Constructor]);
        } else if let Some(i) = ast.cast::<InstanceCreationExpression>(parent) {
            self.constructor_name(ast[i].constructor_name);
        } else if let Some(m) = ast.cast::<MethodInvocation>(parent) {
            self.method_invocation(m);
        }
    }

    /// Dart `buildMatchersForNode`.
    fn build(&mut self, node: NodeId, name_token: TokenId) {
        let c = self.c;
        let ast = c.ast;
        if ast.is::<ArgumentList>(node) {
            self.argument_list(node);
        } else if let Some(cn) = ast.cast::<ConstructorName>(node) {
            self.constructor_name(cn);
        } else if let Some(e) = ast.cast::<ExtensionOverride>(node) {
            let name = c.lexeme(ast[e].name).to_string();
            self.add(vec![name], &[Kind::Extension]);
        } else if let Some(f) = ast.cast::<FunctionDeclaration>(node) {
            self.add(vec![c.lexeme(ast[f].name).to_string()], &[]);
        } else if ast.is::<NamedArgument>(node) {
            if let Some(p) = ast.parent(node) {
                self.argument_list(p);
            }
        } else if let Some(t) = ast.cast::<NamedType>(node) {
            self.named_type(t);
        } else if let Some(p) = ast.cast::<PrefixedIdentifier>(node) {
            self.prefixed_identifier(p);
        } else if let Some(m) = ast.cast::<MethodDeclaration>(node) {
            self.add(vec![c.lexeme(ast[m].name).to_string()], &[Kind::Method]);
        } else if let Some(id) = ast.cast::<SimpleIdentifier>(node) {
            let parent = ast.parent(node);
            if let Some(t) = parent.and_then(|p| ast.cast::<NamedType>(p)) {
                self.named_type(t);
            } else if let Some(m) = parent
                .and_then(|p| ast.cast::<MethodDeclaration>(p))
                .filter(|m| ast[*m].name == name_token)
            {
                self.add(vec![c.lexeme(ast[m].name).to_string()], &[Kind::Method]);
            } else if let Some(m) =
                parent
                    .and_then(|p| ast.cast::<MethodInvocation>(p))
                    .filter(|m| {
                        ast[*m].method_name == id
                            && !self.is_prefix(ast[*m].target.map(|t| t.raw()))
                    })
            {
                self.method_invocation(m);
            } else if let Some(p) = parent
                .and_then(|p| ast.cast::<PrefixedIdentifier>(p))
                .filter(|p| ast[*p].identifier == id)
            {
                self.prefixed_identifier(p);
            } else if let Some(p) = parent
                .and_then(|p| ast.cast::<PropertyAccess>(p))
                .filter(|p| {
                    ast[*p].property_name == id && !self.is_prefix(ast[*p].target.map(|t| t.raw()))
                })
            {
                self.property_access(p);
            } else {
                let element = c.element_of(node);
                if let Some(e) = element.filter(|e| matches!(e.tag(), Tag::Getter | Tag::Setter)) {
                    if let Some(enclosing) = c.ctx.element_data(e).and_then(|d| d.enclosing) {
                        if enclosing.tag() != Tag::Library {
                            if let Some(n) = c.ctx.element_name(enclosing) {
                                self.add(vec![self.name(id), n.to_string()], &[]);
                                return;
                            }
                        }
                    }
                }
                self.add(vec![self.name(id)], &[]);
            }
        } else if let Some(v) = ast.cast::<VariableDeclaration>(node) {
            self.add(vec![c.lexeme(ast[v].name).to_string()], &[]);
        } else if ast.is::<TypeArgumentList>(node) {
            let parent = ast.parent(node);
            if let Some(i) = parent.and_then(|p| ast.cast::<InstanceCreationExpression>(p)) {
                self.constructor_name(ast[i].constructor_name);
            } else if let Some(m) = parent.and_then(|p| ast.cast::<MethodInvocation>(p)) {
                self.method_invocation(m);
            } else if let Some(e) = parent.and_then(|p| ast.cast::<ExtensionOverride>(p)) {
                let name = c.lexeme(ast[e].name).to_string();
                self.add(vec![name], &[Kind::Extension]);
            }
        }
    }
}

/// Dart `DataDriven.producers`.
pub fn data_driven_producers(
    c: &ProducerContext<'_>,
    workspace: &mut dyn ChangeWorkspace,
) -> Vec<Box<dyn CorrectionProducer>> {
    let ctx = c.ctx;
    // Dart `_importElementsForNode`: the URIs of the imported libraries.
    let library = c.resolved.library.library.library;
    let imported: Vec<String> = existing_imports(ctx, library)
        .into_iter()
        .filter_map(|i| i.library.map(|l| library_uri(ctx, l)))
        .collect();
    let mut builder = MatcherBuilder {
        c,
        matchers: Vec::new(),
    };
    builder.build(c.node, c.token);
    if builder.matchers.is_empty() {
        return Vec::new();
    }
    let mut transforms: Vec<Transform> = Vec::new();
    for set in transform_sets(workspace, c.path) {
        for m in &builder.matchers {
            for t in set.iter() {
                if c.applying_bulk_fixes && !t.bulk_apply {
                    continue;
                }
                if matches(m, &imported, &t.element)
                    && !transforms
                        .iter()
                        .any(|x| x.title == t.title && x.element.components == t.element.components)
                {
                    transforms.push(t.clone());
                }
            }
        }
    }
    transforms
        .into_iter()
        .map(|transform| Box::new(DataDrivenFix { transform }) as Box<dyn CorrectionProducer>)
        .collect()
}

/// Dart `DataDrivenFix`.
pub struct DataDrivenFix {
    transform: Transform,
}

impl CorrectionProducer for DataDrivenFix {
    fn fix_kind(&self) -> Option<&'static FixKind> {
        Some(&k::DATA_DRIVEN)
    }

    fn fix_arguments(&self) -> Vec<String> {
        vec![self.transform.title.clone()]
    }

    fn applicability(&self) -> Applicability {
        Applicability::SingleLocation
    }

    fn compute(&mut self, c: &ProducerContext<'_>, builder: &mut ChangeBuilder<'_>) {
        // Dart `Rename.validate` then `Rename.apply` for each change.
        let mut edits: Vec<(u32, u32, String)> = Vec::new();
        for new_name in &self.transform.renames {
            let Some(edit) = rename(c, self.transform.element.kind, new_name) else {
                return;
            };
            edits.extend(edit);
        }
        builder.add_dart_file_edit(c.path, |b| {
            for (offset, length, text) in &edits {
                if *length == 0 {
                    b.add_simple_insertion(*offset, text);
                } else {
                    b.add_simple_replacement(*offset, *length, text);
                }
            }
        });
    }
}

/// Dart `Rename.validate` and `Rename.apply`: the edits.
fn rename(c: &ProducerContext<'_>, kind: Kind, new_name: &str) -> Option<Vec<(u32, u32, String)>> {
    let ast = c.ast;
    let node = c.node;
    // Dart `validate`: the node and the name token.
    let (data_node, name_token): (NodeId, Option<TokenId>) = if let Some(e) =
        ast.cast::<ExtensionOverride>(node)
    {
        (node, Some(ast[e].name))
    } else if let Some(m) = ast.cast::<MethodDeclaration>(node) {
        (node, Some(ast[m].name))
    } else if let Some(t) = ast.cast::<NamedType>(node) {
        match ast
            .parent(node)
            .and_then(|p| ast.cast::<ConstructorName>(p))
        {
            Some(cn) if kind == Kind::Constructor => (cn.raw(), ast[cn].name.map(|n| ast[n].token)),
            _ => (node, Some(ast[t].name)),
        }
    } else if ast.is::<NamedArgument>(node) {
        let target = ast.parent(node).and_then(|p| ast.parent(p))?;
        if let Some(m) = ast.cast::<MethodInvocation>(target) {
            (
                ast[m].method_name.raw(),
                Some(ast[ast[m].method_name].token),
            )
        } else if let Some(i) = ast.cast::<InstanceCreationExpression>(target) {
            let cn = ast[i].constructor_name;
            (cn.raw(), ast[cn].name.map(|n| ast[n].token))
        } else {
            return None;
        }
    } else if let Some(id) = ast.cast::<SimpleIdentifier>(node) {
        (node, Some(ast[id].token))
    } else if let Some(cn) = ast.cast::<ConstructorName>(node) {
        (node, ast[cn].name.map(|n| ast[n].token))
    } else if let Some(p) = ast.cast::<PrefixedIdentifier>(node) {
        (ast[p].identifier.raw(), Some(ast[ast[p].identifier].token))
    } else {
        return None;
    };
    let token_range = |t: TokenId| (c.token_offset(t), ast.tokens.get(t).length);
    let mut out = Vec::new();
    if kind == Kind::Constructor {
        let parent = ast.parent(data_node);
        if let Some(cn) = ast.cast::<ConstructorName>(data_node) {
            if name_token.is_some() && new_name.is_empty() {
                let start = c.token_offset(ast[cn].period?);
                out.push((start, ast.end(cn) - start, String::new()));
            } else if name_token.is_none() && !new_name.is_empty() {
                out.push((ast.end(cn), 0, format!(".{new_name}")));
            } else if let Some(t) = name_token {
                let (o, l) = token_range(t);
                out.push((o, l, new_name.to_string()));
            }
        } else if let Some(t) = name_token {
            if let Some(m) = parent.and_then(|p| ast.cast::<MethodInvocation>(p)) {
                if new_name.is_empty() {
                    let start = c.token_offset(ast[m].operator?);
                    out.push((start, c.token_end(t) - start, String::new()));
                } else {
                    let (o, l) = token_range(t);
                    out.push((o, l, new_name.to_string()));
                }
            } else if let Some(nt) = parent.filter(|p| {
                ast.is::<NamedType>(*p)
                    && ast.parent(*p).is_some_and(|g| ast.is::<ConstructorName>(g))
            }) {
                out.push((ast.end(nt), 0, format!(".{new_name}")));
            } else if let Some(p) = parent.filter(|p| ast.is::<PrefixedIdentifier>(*p)) {
                out.push((ast.end(p), 0, format!(".{new_name}")));
            } else {
                let (o, l) = token_range(t);
                out.push((o, l, new_name.to_string()));
            }
        } else {
            return Some(out);
        }
    } else if let Some(t) = name_token {
        let (o, l) = token_range(t);
        out.push((o, l, new_name.to_string()));
    }
    Some(out)
}

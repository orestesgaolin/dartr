// Dart source: pkg/analysis_server/lib/src/services/correction/dart/data_driven.dart (DataDriven, DataDrivenFix)
// Dart source: pkg/analysis_server/lib/src/services/correction/fix/data_driven/transform_set_manager.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/fix/data_driven/transform_set_parser.dart (the element transforms)
// Dart source: pkg/analysis_server/lib/src/services/correction/fix/data_driven/element_matcher.dart (ElementMatcher, _MatcherBuilder)
// Dart source: pkg/analysis_server/lib/src/services/correction/fix/data_driven/element_kind.dart
// Dart source: pkg/analysis_server/lib/src/services/correction/fix/data_driven/rename.dart

//! Data-driven fixes (`fix_data.yaml`, as `dart fix` applies them).
//!
//! Ported: the element transforms with the changes `rename`,
//! `renameParameter`, `addParameter`, `removeParameter`, `changeParameterType`,
//! `addTypeParameter` and `replacedBy` (code templates with `fragment` and
//! `import` variables, `oneOf` / `if` selectors, and `requiredIf`).
//! Not ported: `library` transforms (the transform gives no fix).

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
use super::data_driven_changes::*;

/// Dart `ElementKind` of `element_kind.dart`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
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

/// Dart `ChangesSelector`.
#[derive(Clone, Debug)]
pub enum ChangesSelector {
    Unconditional(Vec<Change>),
    Conditional(Vec<(ConditionExpr, Vec<Change>)>),
}

/// Dart `Transform`.
#[derive(Clone, Debug)]
pub struct Transform {
    title: String,
    bulk_apply: bool,
    element: ElementDescriptor,
    changes_selector: ChangesSelector,
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

/// Dart `TransformSetParser.parse` (element transforms).
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
        if map_value(t, "date").is_none() {
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
        let transform_scope = parse_variables(map_value(t, "variables"), &[], package_name);
        let Some(changes_selector) =
            parse_changes_selector(t, kind, &components, &transform_scope, package_name)
        else {
            continue;
        };
        out.push(Transform {
            title,
            bulk_apply,
            element: ElementDescriptor {
                library_uris,
                kind,
                components,
            },
            changes_selector,
        });
    }
    out
}

fn parse_change_list(
    changes: &[YamlNode],
    kind: Kind,
    components: &[String],
    scope: &Scope,
    package_name: Option<&str>,
) -> Option<Vec<Change>> {
    let mut parsed: Vec<Change> = Vec::new();
    let mut modifications: Vec<ParameterModification> = Vec::new();
    for change in changes {
        match parse_change(
            change,
            kind,
            components,
            scope,
            package_name,
            &mut modifications,
        ) {
            Ok(Some(c)) => parsed.push(c),
            Ok(None) => {}
            Err(()) => return None,
        }
    }
    if !modifications.is_empty() {
        parsed.push(Change::ModifyParameters(modifications));
    }
    Some(parsed)
}

/// Dart `_translateChangesSelector`.
fn parse_changes_selector(
    t: &YamlNode,
    kind: Kind,
    components: &[String],
    scope: &Scope,
    package_name: Option<&str>,
) -> Option<ChangesSelector> {
    if map_value(t, "changes").is_some() && map_value(t, "oneOf").is_some() {
        return None;
    }
    if let Some(changes) = map_value(t, "changes").and_then(|c| c.as_list()) {
        let parsed = parse_change_list(changes, kind, components, scope, package_name)?;
        return Some(ChangesSelector::Unconditional(parsed));
    }
    if let Some(one_of) = map_value(t, "oneOf").and_then(|o| o.as_list()) {
        let mut table = Vec::new();
        for entry in one_of {
            let cond_str = string_of(map_value(entry, "if"))?;
            let cond = parse_condition(&cond_str, scope)?;
            let changes = map_value(entry, "changes")?.as_list()?;
            let parsed = parse_change_list(changes, kind, components, scope, package_name)?;
            table.push((cond, parsed));
        }
        return Some(ChangesSelector::Conditional(table));
    }
    None
}

/// A variable scope (Dart `VariableScope`): the generators by name.
type Scope = Vec<(String, TemplatePart)>;

/// Dart `TransformSetParser._translateUri`.
fn translate_uri(uri: &str, package_name: Option<&str>) -> String {
    match package_name {
        Some(p) if !uri.starts_with("dart:") && !uri.starts_with("package:") => {
            format!("package:{p}/{uri}")
        }
        _ => uri.to_string(),
    }
}

/// Dart `CodeFragmentParser.parseAccessors`.
fn parse_accessors(text: &str) -> Option<Vec<Accessor>> {
    let mut out = Vec::new();
    for part in text.split('.') {
        let part = part.trim();
        let (name, rest) = part.split_once('[')?;
        let index = rest.strip_suffix(']')?.trim();
        match name.trim() {
            "arguments" => out.push(Accessor::Argument(match index.parse::<usize>() {
                Ok(i) => ParameterReference::Positional(i),
                Err(_) => ParameterReference::Named(index.to_string()),
            })),
            "typeArguments" => out.push(Accessor::TypeArgument(index.parse().ok()?)),
            _ => return None,
        }
    }
    (!out.is_empty()).then_some(out)
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum CondToken {
    And,
    Equal,
    NotEqual,
    Ident(String),
    Literal(String),
}

fn scan_condition_tokens(content: &str) -> Option<Vec<CondToken>> {
    let bytes = content.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut tokens = Vec::new();
    while i < len {
        if matches!(bytes[i], b' ' | b'\n' | b'\r' | b'\t') {
            i += 1;
            continue;
        }
        if bytes[i..].starts_with(b"&&") {
            tokens.push(CondToken::And);
            i += 2;
        } else if bytes[i..].starts_with(b"==") {
            tokens.push(CondToken::Equal);
            i += 2;
        } else if bytes[i..].starts_with(b"!=") {
            tokens.push(CondToken::NotEqual);
            i += 2;
        } else if bytes[i] == b'\'' {
            let start = i + 1;
            let rel_end = content[start..].find('\'')?;
            let end = start + rel_end;
            tokens.push(CondToken::Literal(content[start..end].to_string()));
            i = end + 1;
        } else if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            let start = i;
            i += 1;
            while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            tokens.push(CondToken::Ident(content[start..i].to_string()));
        } else {
            return None;
        }
    }
    Some(tokens)
}

fn parse_cond_primary(
    tokens: &[CondToken],
    pos: &mut usize,
    scope: &Scope,
) -> Option<ConditionExpr> {
    match tokens.get(*pos)? {
        CondToken::Ident(name) => {
            *pos += 1;
            let (_, generator) = scope.iter().find(|(n, _)| n == name)?;
            Some(ConditionExpr::Variable(generator.clone()))
        }
        CondToken::Literal(s) => {
            *pos += 1;
            Some(ConditionExpr::LiteralString(s.clone()))
        }
        _ => None,
    }
}

fn parse_cond_equality(
    tokens: &[CondToken],
    pos: &mut usize,
    scope: &Scope,
) -> Option<ConditionExpr> {
    let left = parse_cond_primary(tokens, pos, scope)?;
    match tokens.get(*pos) {
        Some(CondToken::Equal) => {
            *pos += 1;
            let right = parse_cond_primary(tokens, pos, scope)?;
            Some(ConditionExpr::Binary(
                ConditionOp::Equal,
                Box::new(left),
                Box::new(right),
            ))
        }
        Some(CondToken::NotEqual) => {
            *pos += 1;
            let right = parse_cond_primary(tokens, pos, scope)?;
            Some(ConditionExpr::Binary(
                ConditionOp::NotEqual,
                Box::new(left),
                Box::new(right),
            ))
        }
        _ => Some(left),
    }
}

/// Dart `CodeFragmentParser.parseCondition`.
fn parse_condition(content: &str, scope: &Scope) -> Option<ConditionExpr> {
    let tokens = scan_condition_tokens(content)?;
    let mut pos = 0;
    let mut left = parse_cond_equality(&tokens, &mut pos, scope)?;
    while tokens.get(pos) == Some(&CondToken::And) {
        pos += 1;
        let right = parse_cond_equality(&tokens, &mut pos, scope)?;
        left = ConditionExpr::Binary(ConditionOp::And, Box::new(left), Box::new(right));
    }
    (pos == tokens.len()).then_some(left)
}

/// Dart `_translateTemplateVariables`.
fn parse_variables(
    node: Option<&YamlNode>,
    enclosing: &[(String, TemplatePart)],
    package_name: Option<&str>,
) -> Scope {
    let mut scope: Scope = enclosing.to_vec();
    let Some(entries) = node.and_then(|n| n.as_map()) else {
        return scope;
    };
    for (key, value) in entries {
        let Some(name) = key.string_value() else {
            continue;
        };
        let generator = match string_of(map_value(value, "kind")).as_deref() {
            Some("fragment") => string_of(map_value(value, "value"))
                .and_then(|v| parse_accessors(&v))
                .map(TemplatePart::Fragment),
            Some("import") => {
                let uris: Vec<String> = map_value(value, "uris")
                    .and_then(|u| u.as_list())
                    .map(|l| {
                        l.iter()
                            .filter_map(|u| u.string_value())
                            .map(|u| translate_uri(u, package_name))
                            .collect()
                    })
                    .unwrap_or_default();
                match string_of(map_value(value, "name")) {
                    Some(n) if !uris.is_empty() => Some(TemplatePart::Import { uris, name: n }),
                    _ => None,
                }
            }
            _ => None,
        };
        if let Some(g) = generator {
            scope.insert(0, (name.to_string(), g));
        }
    }
    scope
}

/// Dart `_translateCodeTemplate`.
fn parse_code_template(
    node: Option<&YamlNode>,
    scope: &Scope,
    package_name: Option<&str>,
    can_be_conditionally_required: bool,
) -> Result<Option<CodeTemplate>, ()> {
    let Some(node) = node else { return Ok(None) };
    let Some(template) = string_of(map_value(node, "expression")) else {
        return Err(());
    };
    let scope = parse_variables(map_value(node, "variables"), scope, package_name);
    let required_if = match map_value(node, "requiredIf") {
        Some(req_node) => {
            if !can_be_conditionally_required {
                return Err(());
            }
            let Some(req_str) = string_of(Some(req_node)) else {
                return Err(());
            };
            let Some(cond) = parse_condition(&req_str, &scope) else {
                return Err(());
            };
            Some(cond)
        }
        None => None,
    };
    // Dart `_extractTemplateComponents`.
    let mut parts = Vec::new();
    let mut text_start = 0;
    let mut variable_start = template.find("{%");
    while let Some(start) = variable_start {
        if text_start < start {
            parts.push(TemplatePart::Text(template[text_start..start].to_string()));
        }
        let Some(end) = template[start + 2..].find("%}").map(|e| e + start + 2) else {
            return Ok(Some(CodeTemplate { parts, required_if }));
        };
        let name = template[start + 2..end].trim();
        if let Some((_, g)) = scope.iter().find(|(n, _)| n == name) {
            parts.push(g.clone());
        }
        text_start = end + 2;
        variable_start = template[text_start..].find("{%").map(|s| s + text_start);
    }
    if text_start < template.len() {
        parts.push(TemplatePart::Text(template[text_start..].to_string()));
    }
    Ok(Some(CodeTemplate { parts, required_if }))
}

fn parameter_reference(change: &YamlNode) -> Option<ParameterReference> {
    if let Some(index) = map_value(change, "index").and_then(|i| i.scalar()) {
        return index
            .to_dart_string()
            .parse()
            .ok()
            .map(ParameterReference::Positional);
    }
    string_of(map_value(change, "name")).map(ParameterReference::Named)
}

/// Dart `compatibleReplacementTypes`.
fn compatible_replacement(old: Kind, new: Kind) -> bool {
    use Kind::*;
    let sets: &[&[Kind]] = &[
        &[Constructor],
        &[Function, Method],
        &[Getter, Method],
        &[Constant, Field, Getter, Variable],
        &[Field, Setter, Variable],
        &[Class, Enum, ExtensionType, Mixin, Typedef],
    ];
    sets.iter().any(|s| s.contains(&old) && s.contains(&new))
}

/// Dart `_translateChange`: `Ok(None)` for parameter modifications (added
/// to [modifications]) and invalid changes, `Err` for changes that dartr
/// does not support.
fn parse_change(
    change: &YamlNode,
    kind: Kind,
    _components: &[String],
    scope: &Scope,
    package_name: Option<&str>,
    modifications: &mut Vec<ParameterModification>,
) -> Result<Option<Change>, ()> {
    let Some(change_kind) = string_of(map_value(change, "kind")) else {
        return Ok(None);
    };
    match change_kind.as_str() {
        "rename" => Ok(string_of(map_value(change, "newName")).map(Change::Rename)),
        "renameParameter" => {
            let (Some(old_name), Some(new_name)) = (
                string_of(map_value(change, "oldName")),
                string_of(map_value(change, "newName")),
            ) else {
                return Ok(None);
            };
            Ok(Some(Change::RenameParameter { old_name, new_name }))
        }
        "addParameter" => {
            let index = map_value(change, "index")
                .and_then(|i| i.scalar())
                .and_then(|s| s.to_dart_string().parse::<usize>().ok());
            let name = string_of(map_value(change, "name"));
            let style = string_of(map_value(change, "style"));
            let (Some(index), Some(name), Some(style)) = (index, name, style) else {
                return Ok(None);
            };
            if !matches!(
                style.as_str(),
                "optional_named" | "optional_positional" | "required_named" | "required_positional"
            ) {
                return Ok(None);
            }
            let is_required = style.starts_with("required_");
            let is_positional = style.ends_with("_positional");
            let argument_value = parse_code_template(
                map_value(change, "argumentValue"),
                scope,
                package_name,
                true,
            )?;
            if is_required && argument_value.is_none() {
                return Ok(None);
            }
            if style != "optional_named"
                && argument_value
                    .as_ref()
                    .is_some_and(|v| v.required_if.is_some())
            {
                return Ok(None);
            }
            modifications.push(ParameterModification::Add {
                index,
                name,
                is_required,
                is_positional,
                argument_value,
            });
            Ok(None)
        }
        "removeParameter" => {
            if let Some(reference) = parameter_reference(change) {
                modifications.push(ParameterModification::Remove(reference));
            }
            Ok(None)
        }
        "changeParameterType" => {
            if map_value(change, "index").is_some() && map_value(change, "name").is_some() {
                return Ok(None);
            }
            let Some(reference) = parameter_reference(change) else {
                return Ok(None);
            };
            let Some(nullability) = string_of(map_value(change, "nullability")) else {
                return Ok(None);
            };
            if nullability != "non_null" {
                return Ok(None);
            }
            let argument_value = parse_code_template(
                map_value(change, "argumentValue"),
                scope,
                package_name,
                false,
            )?;
            modifications.push(ParameterModification::ChangeType {
                reference,
                argument_value,
            });
            Ok(None)
        }
        "addTypeParameter" => {
            let index = map_value(change, "index")
                .and_then(|i| i.scalar())
                .and_then(|s| s.to_dart_string().parse::<usize>().ok());
            let name = string_of(map_value(change, "name"));
            let extended_type =
                parse_code_template(map_value(change, "extends"), scope, package_name, false)?;
            let argument_value = parse_code_template(
                map_value(change, "argumentValue"),
                scope,
                package_name,
                false,
            )?;
            let (Some(index), Some(name), Some(argument_value)) = (index, name, argument_value)
            else {
                return Ok(None);
            };
            Ok(Some(Change::AddTypeParameter {
                index,
                name,
                extended_type,
                argument_value,
            }))
        }
        "replacedBy" => {
            let Some(element) = map_value(change, "newElement") else {
                return Ok(None);
            };
            let Some(new_element) = parse_element(element, package_name) else {
                return Ok(None);
            };
            if !compatible_replacement(kind, new_element.kind) {
                return Ok(None);
            }
            let replace_target = map_value(change, "replaceTarget")
                .and_then(|b| b.scalar())
                .is_some_and(|s| s.to_dart_string() == "true");
            let mut arguments = Vec::new();
            if let Some(list) = map_value(change, "arguments").and_then(|l| l.as_list()) {
                for a in list {
                    if let Some(t) = parse_code_template(Some(a), scope, package_name, false)? {
                        arguments.push(t);
                    }
                }
            }
            Ok(Some(Change::ReplacedBy {
                new_element,
                replace_target,
                arguments,
            }))
        }
        _ => Err(()),
    }
}

/// Dart `_translateElement` for the `newElement` of `replacedBy`.
fn parse_element(element: &YamlNode, package_name: Option<&str>) -> Option<NewElement> {
    let uris = map_value(element, "uris")?.as_list()?;
    let library_uris: Vec<String> = uris
        .iter()
        .filter_map(|u| u.string_value())
        .map(|u| translate_uri(u, package_name))
        .collect();
    let mut kind_and_name = None;
    for (key, value) in element.as_map()? {
        if let Some(kind) = key.string_value().and_then(Kind::from_key) {
            kind_and_name = string_of(Some(value)).map(|n| (kind, n));
        }
    }
    let (kind, name) = kind_and_name?;
    let mut components = vec![name];
    if let Some(c) = kind
        .container_keys()
        .iter()
        .find_map(|key| string_of(map_value(element, key)))
    {
        components.push(c);
    }
    if library_uris.is_empty() {
        return None;
    }
    Some(NewElement {
        library_uris,
        kind,
        components,
    })
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
        // Dart `DataDrivenFix.compute`: validate each change, then apply.
        enum Data {
            Edits(Vec<(u32, u32, String)>),
            Modify(Id<ArgumentList>, Vec<ParameterModification>),
            TypeParameter(
                TypeParameterData,
                usize,
                String,
                Option<CodeTemplate>,
                CodeTemplate,
            ),
            Replaced(ReplacedByData, NewElement, bool, Vec<CodeTemplate>),
        }
        let transform = &self.transform;
        let changes: &[Change] = match &transform.changes_selector {
            ChangesSelector::Unconditional(changes) => changes.as_slice(),
            ChangesSelector::Conditional(table) => {
                let context = invocation_of(c.ast, c.node);
                let Some(changes) = table.iter().find_map(|(cond, changes)| {
                    (cond.evaluate_bool(c, context) == Some(true)).then_some(changes.as_slice())
                }) else {
                    return;
                };
                changes
            }
        };
        if changes.is_empty() {
            return;
        }
        let mut data: Vec<Data> = Vec::new();
        for change in changes {
            match change {
                Change::Rename(new_name) => {
                    let Some(edit) = rename(c, transform.element.kind, new_name) else {
                        return;
                    };
                    data.push(Data::Edits(edit));
                }
                Change::RenameParameter { old_name, new_name } => {
                    // Dart `_IgnoredData`: no edit when the change does not apply.
                    let edits = rename_parameter(c, old_name, new_name)
                        .into_iter()
                        .collect();
                    data.push(Data::Edits(edits));
                }
                Change::ModifyParameters(modifications) => {
                    let Some(list) = modify_parameters_argument_list(c) else {
                        return;
                    };
                    data.push(Data::Modify(list, modifications.clone()));
                }
                Change::AddTypeParameter {
                    index,
                    name,
                    extended_type,
                    argument_value,
                } => {
                    let Some(d) = add_type_parameter_validate(
                        c,
                        *index,
                        extended_type.as_ref(),
                        argument_value,
                    ) else {
                        return;
                    };
                    data.push(Data::TypeParameter(
                        d,
                        *index,
                        name.clone(),
                        extended_type.clone(),
                        argument_value.clone(),
                    ));
                }
                Change::ReplacedBy {
                    new_element,
                    replace_target,
                    arguments,
                } => {
                    let Some(d) = replaced_by_validate(
                        c,
                        &transform.element.components,
                        new_element.kind,
                        *replace_target,
                    ) else {
                        return;
                    };
                    data.push(Data::Replaced(
                        d,
                        new_element.clone(),
                        *replace_target,
                        arguments.clone(),
                    ));
                }
            }
        }
        builder.add_dart_file_edit(c.path, |b| {
            for d in &data {
                match d {
                    Data::Edits(edits) => {
                        for (offset, length, text) in edits {
                            if *length == 0 {
                                b.add_simple_insertion(*offset, text);
                            } else {
                                b.add_simple_replacement(*offset, *length, text);
                            }
                        }
                    }
                    Data::Modify(list, modifications) => {
                        apply_modify_parameters(c, b, *list, modifications);
                    }
                    Data::TypeParameter(d, index, name, extended_type, argument_value) => {
                        add_type_parameter_apply(
                            c,
                            b,
                            d,
                            *index,
                            name,
                            extended_type.as_ref(),
                            argument_value,
                        );
                    }
                    Data::Replaced(d, new_element, replace_target, arguments) => {
                        replaced_by_apply(c, b, d, new_element, *replace_target, arguments);
                    }
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

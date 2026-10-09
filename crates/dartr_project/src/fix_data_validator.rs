//! Validation of `fix_data.yaml` files.
//!
//! Port of the diagnostics of
//! `pkg/analysis_server/lib/src/services/correction/fix/data_driven/transform_set_parser.dart`
//! and `code_fragment_parser.dart` (diagnostic codes: `TransformSetErrorCode`
//! in `pkg/analysis_server/messages.yaml`), as called by
//! `ContextManagerImpl._analyzeFixDataYaml`.
//!
//! The data model (`Transform`, `Change`, `ElementDescriptor`, ...) is not
//! built: the checks only need to know whether a node translated
//! successfully, the kind of the element, the names of the template variables
//! and the number of translated list elements. Everything that Dart reports
//! is reported here in the same order, at the same range.
//!
//! Dart throws exceptions in a few places (`RangeError` in the code fragment
//! scanner, `FormatException` of `Uri.parse` and `int.parse`). The caller
//! catches every exception and publishes no diagnostics for the file. This
//! port does the same: [`validate_fix_data`] returns an empty list when such a
//! case is detected (`Parser::aborted`).

use dartr_diagnostics::{Diagnostic, LocatableDiagnostic, diag};
use dartr_yaml::{FileSpan, NodeKind, Scalar, ScalarStyle, YamlNode};
use std::collections::HashSet;

const ARGUMENTS: &str = "arguments";
const ARGUMENT_VALUE_KEY: &str = "argumentValue";
const BULK_APPLY_KEY: &str = "bulkApply";
const CHANGES_KEY: &str = "changes";
const CLASS_KEY: &str = "class";
const CONSTANT_KEY: &str = "constant";
const CONSTRUCTOR_KEY: &str = "constructor";
const DATE_KEY: &str = "date";
const ELEMENT_KEY: &str = "element";
const ENUM_KEY: &str = "enum";
const EXPRESSION_KEY: &str = "expression";
const EXTENDS_KEY: &str = "extends";
const EXTENSION_KEY: &str = "extension";
const EXTENSION_TYPE_KEY: &str = "extensionType";
const FIELD_KEY: &str = "field";
const FUNCTION_KEY: &str = "function";
const GETTER_KEY: &str = "getter";
const IF_KEY: &str = "if";
const IN_CLASS_KEY: &str = "inClass";
const IN_ENUM_KEY: &str = "inEnum";
const IN_EXTENSION_KEY: &str = "inExtension";
const IN_EXTENSION_TYPE_KEY: &str = "inExtensionType";
const INDEX_KEY: &str = "index";
const IN_MIXIN_KEY: &str = "inMixin";
const KIND_KEY: &str = "kind";
const LIBRARY_KEY: &str = "library";
const METHOD_KEY: &str = "method";
const MIXIN_KEY: &str = "mixin";
const NAME_KEY: &str = "name";
const NEW_ELEMENT_KEY: &str = "newElement";
const NEW_LIBRARY_KEY: &str = "newLibrary";
const NEW_NAME_KEY: &str = "newName";
const NULLABILITY_KEY: &str = "nullability";
const OLD_NAME_KEY: &str = "oldName";
const ONE_OF_KEY: &str = "oneOf";
const REPLACE_TARGET: &str = "replaceTarget";
const REQUIRED_IF_KEY: &str = "requiredIf";
const SETTER_KEY: &str = "setter";
const STATIC_KEY: &str = "static";
const STYLE_KEY: &str = "style";
const TITLE_KEY: &str = "title";
const TRANSFORMS_KEY: &str = "transforms";
const TYPEDEF_KEY: &str = "typedef";
const URIS_KEY: &str = "uris";
const VALUE_KEY: &str = "value";
const VARIABLE_KEY: &str = "variable";
const VARIABLES_KEY: &str = "variables";
const VERSION_KEY: &str = "version";

const ADD_PARAMETER_KIND: &str = "addParameter";
const ADD_TYPE_PARAMETER_KIND: &str = "addTypeParameter";
const CHANGE_PARAMETER_TYPE_KIND: &str = "changeParameterType";
const FRAGMENT_KIND: &str = "fragment";
const IMPORT_KIND: &str = "import";
const REMOVE_PARAMETER_KIND: &str = "removeParameter";
const RENAME_KIND: &str = "rename";
const RENAME_PARAMETER_KIND: &str = "renameParameter";
const REPLACED_BY_KIND: &str = "replacedBy";

const VALID_STYLES: [&str; 4] = [
    "optional_named",
    "optional_positional",
    "required_named",
    "required_positional",
];
const VALID_NULLABILITY_CHANGES: [&str; 1] = ["non_null"];
const CURRENT_VERSION: i64 = 1;

/// `ElementKind` of `element_kind.dart`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ElementKind {
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

impl ElementKind {
    fn display_name(self) -> &'static str {
        match self {
            ElementKind::Class => "class",
            ElementKind::Constant => "constant",
            ElementKind::Constructor => "constructor",
            ElementKind::Enum => "enum",
            ElementKind::Extension => "extension",
            ElementKind::ExtensionType => "extensionType",
            ElementKind::Field => "field",
            ElementKind::Function => "function",
            ElementKind::Getter => "getter",
            ElementKind::Library => "library",
            ElementKind::Method => "method",
            ElementKind::Mixin => "mixin",
            ElementKind::Setter => "setter",
            ElementKind::Typedef => "typedef",
            ElementKind::Variable => "variable",
        }
    }

    fn from_name(name: &str) -> Option<ElementKind> {
        use ElementKind::*;
        [
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
        ]
        .into_iter()
        .find(|kind| kind.display_name() == name)
    }

    /// `compatibleReplacementTypes[this]`: `None` when the kind has no entry.
    fn compatible_replacements(self) -> Option<Vec<ElementKind>> {
        use ElementKind::*;
        let sets: [&[ElementKind]; 6] = [
            &[Constructor],
            &[Function, Method],
            &[Getter, Method],
            &[Constant, Field, Getter, Variable],
            &[Field, Setter, Variable],
            &[Class, Enum, ExtensionType, Mixin, Typedef],
        ];
        let mut result: Vec<ElementKind> = Vec::new();
        let mut found = false;
        for set in sets {
            if set.contains(&self) {
                found = true;
                for kind in set {
                    if !result.contains(kind) {
                        result.push(*kind);
                    }
                }
            }
        }
        found.then_some(result)
    }
}

/// The keys of the possible containers of a member element
/// (`_containerKeyMap`), in declaration order.
fn container_keys(element_key: &str) -> Option<&'static [&'static str]> {
    Some(match element_key {
        CONSTRUCTOR_KEY => &[IN_CLASS_KEY, IN_EXTENSION_TYPE_KEY],
        CONSTANT_KEY => &[IN_ENUM_KEY],
        FIELD_KEY => &[
            IN_CLASS_KEY,
            IN_EXTENSION_KEY,
            IN_EXTENSION_TYPE_KEY,
            IN_MIXIN_KEY,
        ],
        GETTER_KEY | METHOD_KEY | SETTER_KEY => &[
            IN_CLASS_KEY,
            IN_EXTENSION_KEY,
            IN_EXTENSION_TYPE_KEY,
            IN_MIXIN_KEY,
        ],
        _ => return None,
    })
}

const ELEMENT_KEYS: [&str; 14] = [
    CLASS_KEY,
    CONSTANT_KEY,
    CONSTRUCTOR_KEY,
    ENUM_KEY,
    EXTENSION_KEY,
    EXTENSION_TYPE_KEY,
    FIELD_KEY,
    FUNCTION_KEY,
    GETTER_KEY,
    METHOD_KEY,
    MIXIN_KEY,
    SETTER_KEY,
    TYPEDEF_KEY,
    VARIABLE_KEY,
];

/// `quotedAndCommaSeparatedWithOr` of `utilities/extensions/string.dart`.
fn quoted_with_or<S: AsRef<str>>(items: &[S]) -> String {
    let quote = |s: &S| format!("'{}'", s.as_ref());
    match items.len() {
        0 => String::new(),
        1 => quote(&items[0]),
        2 => format!("{} or {}", quote(&items[0]), quote(&items[1])),
        n => {
            let head: Vec<String> = items[..n - 1].iter().map(quote).collect();
            format!("{}, or {}", head.join(", "), quote(&items[n - 1]))
        }
    }
}

/// `ErrorContext`.
#[derive(Clone, Copy)]
struct Ctx<'a> {
    key: &'a str,
    parent: FileSpan,
}

fn ctx<'a>(key: &'a str, parent: &YamlNode) -> Ctx<'a> {
    Ctx {
        key,
        parent: parent.span,
    }
}

/// `_nodeType`.
fn node_type(node: &YamlNode) -> &'static str {
    match &node.kind {
        NodeKind::Scalar(Scalar::Null) => "Null",
        NodeKind::Scalar(Scalar::Bool(_)) => "bool",
        NodeKind::Scalar(Scalar::Int(_)) => "int",
        NodeKind::Scalar(Scalar::Float(_)) => "double",
        NodeKind::Scalar(Scalar::String(_)) => "String",
        NodeKind::List(_) => "List",
        NodeKind::Map(_) => "Map",
    }
}

/// `_offsetOfString`.
fn offset_of_string(node: &YamlNode) -> usize {
    if node.scalar_style == ScalarStyle::Plain {
        node.span.start.offset
    } else {
        node.span.start.offset + 1
    }
}

/// `YamlMapExtensions.getKey`.
fn get_key<'a>(map: &'a YamlNode, key: &str) -> Option<&'a YamlNode> {
    let NodeKind::Map(entries) = &map.kind else {
        return None;
    };
    entries
        .iter()
        .find(|(k, _)| matches!(&k.kind, NodeKind::Scalar(Scalar::String(s)) if s == key))
        .map(|(k, _)| k)
}

fn is_map(node: &YamlNode) -> bool {
    matches!(node.kind, NodeKind::Map(_))
}

/// A set of variable names (`VariableScope`).
type Scope = HashSet<String>;

/// Validates the text of a `fix_data.yaml` file. [package_name] is the short
/// name of the context root folder (`_analyzeFixDataYaml`).
pub fn validate_fix_data(text: &str, package_name: &str) -> Vec<Diagnostic> {
    let mut parser = Parser {
        source: text.encode_utf16().collect(),
        diagnostics: Vec::new(),
        aborted: false,
        package_name,
        element_being_transformed: None,
        transform_scope: Scope::new(),
    };
    parser.parse(text);
    if parser.aborted {
        return Vec::new();
    }
    parser.diagnostics
}

struct Parser<'p> {
    /// The file content in UTF-16 code units (the unit of the offsets).
    source: Vec<u16>,
    diagnostics: Vec<Diagnostic>,
    /// An exception was thrown in Dart: no diagnostics are published.
    aborted: bool,
    package_name: &'p str,
    element_being_transformed: Option<ElementKind>,
    transform_scope: Scope,
}

/// The result of translating a code template (`CodeTemplate`).
struct Template {
    has_required_if: bool,
}

impl Parser<'_> {
    /// `LocatableDiagnostic.atSourceSpan`: the range of the span without its
    /// trailing line terminators.
    fn report(&mut self, diagnostic: LocatableDiagnostic, span: FileSpan) {
        let mut end = span.end.offset;
        while end > span.start.offset && matches!(self.source.get(end - 1), Some(0x0A | 0x0D)) {
            end -= 1;
        }
        self.diagnostics
            .push(diagnostic.to_diagnostic(span.start.offset, end - span.start.offset));
    }

    fn report_at(&mut self, diagnostic: LocatableDiagnostic, offset: usize, length: usize) {
        self.diagnostics
            .push(diagnostic.to_diagnostic(offset, length));
    }

    fn parse(&mut self, content: &str) {
        let Some(node) = self.parse_yaml(content) else {
            return;
        };
        self.translate_transform_set(&node);
    }

    /// `_parseYaml`.
    fn parse_yaml(&mut self, content: &str) -> Option<YamlNode> {
        match dartr_yaml::load_yaml_node(content) {
            Ok(node) => Some(node),
            Err(error) => {
                if error.runtime_error.is_some() {
                    // Not a `YamlException`: caught by the caller.
                    self.aborted = true;
                } else {
                    // `atOffset`: the range is not trimmed.
                    let diagnostic = diag::yaml_syntax_error(&error.message);
                    self.report_at(diagnostic, error.span.start.offset, error.span.length());
                }
                None
            }
        }
    }

    // ---- reporting helpers -------------------------------------------------

    /// `_reportInvalidValue`.
    fn report_invalid_value(&mut self, node: &YamlNode, context: Ctx, expected: &str) {
        let diagnostic = diag::invalid_value(context.key, expected, node_type(node));
        self.report(diagnostic, node.span);
    }

    /// `_reportInvalidValueOneOf`.
    fn report_invalid_value_one_of(&mut self, node: &YamlNode, context: Ctx, allowed: &[&str]) {
        let diagnostic = diag::invalid_value_one_of(context.key, &quoted_with_or(allowed));
        self.report(diagnostic, node.span);
    }

    /// `_reportMissingKey`.
    fn report_missing_key(&mut self, context: Ctx) {
        self.report(diag::missing_key(context.key), context.parent);
    }

    /// `_translateKey`.
    fn translate_key(&mut self, node: &YamlNode) -> Option<String> {
        if let NodeKind::Scalar(Scalar::String(value)) = &node.kind {
            return Some(value.clone());
        }
        self.report(diag::invalid_key(node_type(node)), node.span);
        None
    }

    /// `_reportUnsupportedKeys`.
    fn report_unsupported_keys(&mut self, map: &YamlNode, valid: &[&str]) {
        let NodeKind::Map(entries) = &map.kind else {
            return;
        };
        for (key_node, _) in entries {
            if let Some(key) = self.translate_key(key_node)
                && !valid.contains(&key.as_str())
            {
                self.report(diag::unsupported_key(&key), key_node.span);
            }
        }
    }

    /// `_singleKey`: the key and the value of the first entry whose key is in
    /// one of the [groups]. Extra entries are reported as conflicts. Missing
    /// keys are reported at [error_span] when [required].
    fn single_key<'n>(
        &mut self,
        map: &'n YamlNode,
        groups: &[&[&str]],
        error_span: FileSpan,
        required: bool,
    ) -> Option<(String, &'n YamlNode)> {
        let NodeKind::Map(map_entries) = &map.kind else {
            return None;
        };
        let mut entries: Vec<(&YamlNode, &YamlNode, &str)> = Vec::new();
        for (key_node, value_node) in map_entries {
            if let NodeKind::Scalar(Scalar::String(key)) = &key_node.kind
                && groups.iter().any(|group| group.contains(&key.as_str()))
            {
                entries.push((key_node, value_node, key));
            }
        }
        let Some(&(_, first_value, first_key)) = entries.first() else {
            if required {
                let all: Vec<&str> = groups.iter().flat_map(|g| g.iter().copied()).collect();
                self.report(
                    diag::missing_one_of_multiple_keys(&quoted_with_or(&all)),
                    error_span,
                );
            }
            return None;
        };
        for (key_node, _, key) in entries.iter().skip(1) {
            self.report(diag::conflicting_key(key, first_key), key_node.span);
        }
        Some((first_key.to_string(), first_value))
    }

    // ---- scalar translators --------------------------------------------------

    /// `_translateString`.
    fn translate_string(
        &mut self,
        node: Option<&YamlNode>,
        context: Ctx,
        required: bool,
    ) -> Option<String> {
        match node {
            Some(node) => {
                if let NodeKind::Scalar(Scalar::String(value)) = &node.kind {
                    return Some(value.clone());
                }
                self.report_invalid_value(node, context, "String");
                None
            }
            None => {
                if required {
                    self.report_missing_key(context);
                }
                None
            }
        }
    }

    /// `_translateBool`.
    fn translate_bool(
        &mut self,
        node: Option<&YamlNode>,
        context: Ctx,
        required: bool,
    ) -> Option<bool> {
        match node {
            Some(node) => {
                if let NodeKind::Scalar(Scalar::Bool(value)) = &node.kind {
                    return Some(*value);
                }
                self.report_invalid_value(node, context, "boolean");
                None
            }
            None => {
                if required {
                    self.report_missing_key(context);
                }
                None
            }
        }
    }

    /// `_translateInteger`.
    fn translate_integer(&mut self, node: Option<&YamlNode>, context: Ctx) -> Option<i64> {
        match node {
            Some(node) => {
                if let NodeKind::Scalar(Scalar::Int(value)) = &node.kind {
                    return Some(*value);
                }
                self.report_invalid_value(node, context, "int");
                None
            }
            None => {
                self.report_missing_key(context);
                None
            }
        }
    }

    /// `_translateDate`.
    fn translate_date(&mut self, node: Option<&YamlNode>, context: Ctx) -> Option<()> {
        match node {
            Some(node) => {
                if let NodeKind::Scalar(Scalar::String(value)) = &node.kind
                    && date_time_parses(value)
                {
                    return Some(());
                }
                self.report_invalid_value(node, context, "Date");
                None
            }
            None => {
                self.report_missing_key(context);
                None
            }
        }
    }

    /// `_translateUri`.
    fn translate_uri(
        &mut self,
        node: Option<&YamlNode>,
        context: Ctx,
        required: bool,
    ) -> Option<()> {
        match node {
            Some(node) => {
                if let NodeKind::Scalar(Scalar::String(value)) = &node.kind {
                    let value = if !value.starts_with("dart:") && !value.starts_with("package:") {
                        format!("package:{}/{value}", self.package_name)
                    } else {
                        value.clone()
                    };
                    if !uri_parses(&value) {
                        // `Uri.parse` throws a `FormatException`.
                        self.aborted = true;
                        return None;
                    }
                    return Some(());
                }
                self.report_invalid_value(node, context, "URI");
                None
            }
            None => {
                if required {
                    self.report_missing_key(context);
                }
                None
            }
        }
    }

    /// `_translateUri` as an element translator of `_translateList`.
    fn translate_uri_element(&mut self, node: &YamlNode, context: &Ctx) -> Option<()> {
        self.translate_uri(Some(node), *context, true)
    }

    /// `_translateList`.
    fn translate_list<R>(
        &mut self,
        node: Option<&YamlNode>,
        context: Ctx,
        translator: fn(&mut Self, &YamlNode, &Ctx) -> Option<R>,
    ) -> Option<Vec<R>> {
        match node {
            Some(node) => {
                if let NodeKind::List(elements) = &node.kind {
                    let mut result = Vec::new();
                    for element in elements {
                        if let Some(value) = translator(self, element, &context) {
                            result.push(value);
                        }
                    }
                    Some(result)
                } else {
                    self.report_invalid_value(node, context, "List");
                    None
                }
            }
            None => {
                self.report_missing_key(context);
                None
            }
        }
    }

    // ---- code templates ----------------------------------------------------

    /// `_extractTemplateComponents`: only the diagnostics.
    fn extract_template_components(
        &mut self,
        template: &str,
        scope: &Scope,
        template_offset: usize,
    ) {
        let template: Vec<u16> = template.encode_utf16().collect();
        let open: Vec<u16> = "{%".encode_utf16().collect();
        let close: Vec<u16> = "%}".encode_utf16().collect();
        let mut variable_start = index_of(&template, &open, 0);
        while let Some(start) = variable_start {
            let Some(end) = index_of(&template, &close, start + 2) else {
                self.report_at(diag::missing_template_end(), template_offset + start, 2);
                return;
            };
            let raw = String::from_utf16_lossy(&template[start + 2..end]);
            let name = raw.trim();
            if !scope.contains(name) {
                let name_units: Vec<u16> = name.encode_utf16().collect();
                let at = index_of(&template, &name_units, start).unwrap_or(start);
                self.report_at(
                    diag::undefined_variable(name),
                    template_offset + at,
                    name_units.len(),
                );
            }
            variable_start = index_of(&template, &open, end + 2);
        }
    }

    /// `_translateCodeTemplate`.
    fn translate_code_template(
        &mut self,
        node: Option<&YamlNode>,
        context: Ctx,
        can_be_conditionally_required: bool,
        required: bool,
    ) -> Option<Template> {
        match node {
            Some(node) if is_map(node) => {
                if can_be_conditionally_required {
                    self.report_unsupported_keys(
                        node,
                        &[EXPRESSION_KEY, REQUIRED_IF_KEY, VARIABLES_KEY],
                    );
                } else {
                    self.report_unsupported_keys(node, &[EXPRESSION_KEY, VARIABLES_KEY]);
                }
                let expression_node = node.value_at(EXPRESSION_KEY);
                let template =
                    self.translate_string(expression_node, ctx(EXPRESSION_KEY, node), true);
                let scope = self.translate_template_variables(
                    node.value_at(VARIABLES_KEY),
                    ctx(VARIABLES_KEY, node),
                );
                let mut has_required_if = false;
                if can_be_conditionally_required {
                    let required_if_node = node.value_at(REQUIRED_IF_KEY);
                    let required_if_text =
                        self.translate_string(required_if_node, ctx(REQUIRED_IF_KEY, node), false);
                    if let (Some(required_if_node), Some(text)) =
                        (required_if_node, required_if_text)
                        && matches!(required_if_node.kind, NodeKind::Scalar(_))
                    {
                        let condition =
                            self.parse_condition(&text, offset_of_string(required_if_node), &scope);
                        if condition.is_none() {
                            return None;
                        }
                        has_required_if = true;
                    }
                }
                let expression_node = expression_node?;
                if !matches!(expression_node.kind, NodeKind::Scalar(_)) {
                    return None;
                }
                let template = template?;
                self.extract_template_components(
                    &template,
                    &scope,
                    offset_of_string(expression_node),
                );
                Some(Template { has_required_if })
            }
            None => {
                if required {
                    self.report_missing_key(context);
                }
                None
            }
            Some(node) => {
                self.report_invalid_value(node, context, "Map");
                None
            }
        }
    }

    /// `_translateCodeTemplate` as an element translator of `_translateList`.
    fn translate_code_template_element(
        &mut self,
        node: &YamlNode,
        context: &Ctx,
    ) -> Option<Template> {
        self.translate_code_template(Some(node), *context, false, true)
    }

    /// `_translateTemplateVariables`.
    fn translate_template_variables(&mut self, node: Option<&YamlNode>, context: Ctx) -> Scope {
        match node {
            Some(node) => {
                if let NodeKind::Map(entries) = &node.kind {
                    let mut scope = self.transform_scope.clone();
                    for (key_node, value_node) in entries {
                        if let Some(name) = self.translate_key(key_node)
                            && self
                                .translate_value_generator(Some(value_node), ctx(&name, node))
                                .is_some()
                        {
                            scope.insert(name);
                        }
                    }
                    scope
                } else {
                    self.report_invalid_value(node, context, "Map");
                    self.transform_scope.clone()
                }
            }
            None => self.transform_scope.clone(),
        }
    }

    /// `_translateValueGenerator`.
    fn translate_value_generator(&mut self, node: Option<&YamlNode>, context: Ctx) -> Option<()> {
        match node {
            Some(node) if is_map(node) => {
                let kind_node = node.value_at(KIND_KEY);
                let kind_context = ctx(KIND_KEY, node);
                let kind = self.translate_string(kind_node, kind_context, true);
                let (Some(kind_node), Some(kind)) = (kind_node, kind) else {
                    return None;
                };
                if kind == FRAGMENT_KIND {
                    self.translate_code_fragment(node)
                } else if kind == IMPORT_KIND {
                    self.translate_import_value(node)
                } else {
                    self.report_invalid_value_one_of(
                        kind_node,
                        kind_context,
                        &[FRAGMENT_KIND, IMPORT_KIND],
                    );
                    None
                }
            }
            None => {
                self.report_missing_key(context);
                None
            }
            Some(node) => {
                self.report_invalid_value(node, context, "Map");
                None
            }
        }
    }

    /// `_translateCodeFragment`.
    fn translate_code_fragment(&mut self, node: &YamlNode) -> Option<()> {
        self.report_unsupported_keys(node, &[KIND_KEY, VALUE_KEY]);
        let value_node = node.value_at(VALUE_KEY);
        let value = self.translate_string(value_node, ctx(VALUE_KEY, node), true);
        let value_node = value_node?;
        if !matches!(value_node.kind, NodeKind::Scalar(_)) {
            return None;
        }
        let value = value?;
        self.parse_accessors(&value, offset_of_string(value_node))
    }

    /// `_translateImportValue`.
    fn translate_import_value(&mut self, node: &YamlNode) -> Option<()> {
        self.report_unsupported_keys(node, &[KIND_KEY, NAME_KEY, URIS_KEY]);
        let uris_node = node.value_at(URIS_KEY);
        let uris = self.translate_list(uris_node, ctx(URIS_KEY, node), Self::translate_uri_element);
        let name = self.translate_string(node.value_at(NAME_KEY), ctx(NAME_KEY, node), true);
        let (uris, _name) = (uris?, name?);
        if uris.is_empty() {
            if let Some(uris_node) = uris_node
                && matches!(&uris_node.kind, NodeKind::List(list) if list.is_empty())
            {
                self.report(diag::missing_uri(), uris_node.span);
            }
            return None;
        }
        Some(())
    }

    // ---- changes -------------------------------------------------------------

    /// `_translateAddParameterChange`.
    fn translate_add_parameter_change(&mut self, node: &YamlNode) {
        self.report_unsupported_keys(
            node,
            &[ARGUMENT_VALUE_KEY, INDEX_KEY, KIND_KEY, NAME_KEY, STYLE_KEY],
        );
        if self
            .translate_integer(node.value_at(INDEX_KEY), ctx(INDEX_KEY, node))
            .is_none()
        {
            return;
        }
        if self
            .translate_string(node.value_at(NAME_KEY), ctx(NAME_KEY, node), true)
            .is_none()
        {
            return;
        }
        let style_node = node.value_at(STYLE_KEY);
        let style = self.translate_string(style_node, ctx(STYLE_KEY, node), true);
        let (Some(style_node), Some(style)) = (style_node, style) else {
            return;
        };
        if !VALID_STYLES.contains(&style.as_str()) {
            self.report(
                diag::invalid_parameter_style(&quoted_with_or(&VALID_STYLES)),
                style_node.span,
            );
            return;
        }
        let is_required = style.starts_with("required_");
        let argument_value = self.translate_code_template(
            node.value_at(ARGUMENT_VALUE_KEY),
            ctx(ARGUMENT_VALUE_KEY, node),
            true,
            true,
        );
        match argument_value {
            None if is_required => {}
            Some(template) if template.has_required_if && style != "optional_named" => {
                if let Some(value_node) = node.value_at(ARGUMENT_VALUE_KEY)
                    && let Some(key) = get_key(value_node, REQUIRED_IF_KEY)
                {
                    self.report(diag::invalid_required_if(), key.span);
                }
            }
            _ => {}
        }
    }

    /// `_translateAddTypeParameterChange`.
    fn translate_add_type_parameter_change(&mut self, node: &YamlNode) {
        self.report_unsupported_keys(
            node,
            &[
                EXTENDS_KEY,
                INDEX_KEY,
                KIND_KEY,
                NAME_KEY,
                ARGUMENT_VALUE_KEY,
            ],
        );
        let _ = self.translate_integer(node.value_at(INDEX_KEY), ctx(INDEX_KEY, node));
        let _ = self.translate_string(node.value_at(NAME_KEY), ctx(NAME_KEY, node), true);
        let _ = self.translate_code_template(
            node.value_at(EXTENDS_KEY),
            ctx(EXTENDS_KEY, node),
            false,
            false,
        );
        let _ = self.translate_code_template(
            node.value_at(ARGUMENT_VALUE_KEY),
            ctx(ARGUMENT_VALUE_KEY, node),
            false,
            true,
        );
    }

    /// The `index` / `name` reference of `_translateRemoveParameterChange`
    /// and `_translateChangeParameterTypeChange`.
    fn translate_parameter_reference(&mut self, node: &YamlNode) -> Option<()> {
        let (key, value) = self.single_key(node, &[&[INDEX_KEY], &[NAME_KEY]], node.span, true)?;
        if key == INDEX_KEY {
            self.translate_integer(Some(value), ctx(INDEX_KEY, node))
                .map(|_| ())
        } else {
            self.translate_string(Some(value), ctx(NAME_KEY, node), true)
                .map(|_| ())
        }
    }

    /// `_translateChangeParameterTypeChange`.
    fn translate_change_parameter_type_change(&mut self, node: &YamlNode) {
        self.report_unsupported_keys(
            node,
            &[
                ARGUMENT_VALUE_KEY,
                INDEX_KEY,
                KIND_KEY,
                NAME_KEY,
                NULLABILITY_KEY,
            ],
        );
        if self.translate_parameter_reference(node).is_none() {
            return;
        }
        let nullability_node = node.value_at(NULLABILITY_KEY);
        let Some(nullability) =
            self.translate_string(nullability_node, ctx(NULLABILITY_KEY, node), true)
        else {
            return;
        };
        if !VALID_NULLABILITY_CHANGES.contains(&nullability.as_str()) {
            if let Some(nullability_node) = nullability_node {
                self.report(
                    diag::invalid_value_one_of(
                        NULLABILITY_KEY,
                        &quoted_with_or(&VALID_NULLABILITY_CHANGES),
                    ),
                    nullability_node.span,
                );
            }
            return;
        }
        let _ = self.translate_code_template(
            node.value_at(ARGUMENT_VALUE_KEY),
            ctx(ARGUMENT_VALUE_KEY, node),
            false,
            true,
        );
    }

    /// `_translateChange`.
    fn translate_change(&mut self, node: &YamlNode, context: &Ctx) -> Option<()> {
        if !is_map(node) {
            self.report_invalid_value(node, *context, "Map");
            return None;
        }
        let kind_node = node.value_at(KIND_KEY);
        let kind_context = ctx(KIND_KEY, node);
        let kind = self.translate_string(kind_node, kind_context, true);
        let (Some(kind_node), Some(kind)) = (kind_node, kind) else {
            return None;
        };
        match kind.as_str() {
            ADD_PARAMETER_KIND => {
                self.translate_add_parameter_change(node);
                None
            }
            ADD_TYPE_PARAMETER_KIND => {
                self.translate_add_type_parameter_change(node);
                Some(())
            }
            REMOVE_PARAMETER_KIND => {
                self.report_unsupported_keys(node, &[INDEX_KEY, KIND_KEY, NAME_KEY]);
                let _ = self.translate_parameter_reference(node);
                None
            }
            CHANGE_PARAMETER_TYPE_KIND => {
                self.translate_change_parameter_type_change(node);
                None
            }
            RENAME_KIND => {
                self.report_unsupported_keys(node, &[KIND_KEY, NEW_NAME_KEY]);
                self.translate_string(node.value_at(NEW_NAME_KEY), ctx(NEW_NAME_KEY, node), true)
                    .map(|_| ())
            }
            RENAME_PARAMETER_KIND => {
                self.report_unsupported_keys(node, &[KIND_KEY, NEW_NAME_KEY, OLD_NAME_KEY]);
                let old = self.translate_string(
                    node.value_at(OLD_NAME_KEY),
                    ctx(OLD_NAME_KEY, node),
                    true,
                );
                let new = self.translate_string(
                    node.value_at(NEW_NAME_KEY),
                    ctx(NEW_NAME_KEY, node),
                    true,
                );
                (old.is_some() && new.is_some()).then_some(())
            }
            REPLACED_BY_KIND => self.translate_replaced_by_change(node),
            _ => {
                self.report_invalid_value_one_of(
                    kind_node,
                    kind_context,
                    &[
                        ADD_PARAMETER_KIND,
                        ADD_TYPE_PARAMETER_KIND,
                        REMOVE_PARAMETER_KIND,
                        RENAME_KIND,
                        RENAME_PARAMETER_KIND,
                        REPLACED_BY_KIND,
                    ],
                );
                None
            }
        }
    }

    /// `_translateReplacedByChange`.
    fn translate_replaced_by_change(&mut self, node: &YamlNode) -> Option<()> {
        self.report_unsupported_keys(
            node,
            &[ARGUMENTS, KIND_KEY, NEW_ELEMENT_KEY, REPLACE_TARGET],
        );
        let new_element =
            self.translate_element(node.value_at(NEW_ELEMENT_KEY), ctx(NEW_ELEMENT_KEY, node))?;
        if let Some(old) = self.element_being_transformed {
            let new_element_span = node.value_at(NEW_ELEMENT_KEY)?.span;
            match old.compatible_replacements() {
                None => {
                    self.report(
                        diag::invalid_change_for_kind(REPLACED_BY_KIND, old.display_name()),
                        new_element_span,
                    );
                    return None;
                }
                Some(compatible) if !compatible.contains(&new_element) => {
                    self.report(
                        diag::incompatible_element_kind(
                            old.display_name(),
                            new_element.display_name(),
                        ),
                        new_element_span,
                    );
                    return None;
                }
                Some(_) => {}
            }
        }
        if let Some(replace_target_node) = node.value_at(REPLACE_TARGET) {
            let _ = self.translate_bool(Some(replace_target_node), ctx(REPLACE_TARGET, node), true);
        }
        if let Some(arguments_node) = node.value_at(ARGUMENTS) {
            let _ = self.translate_list(
                Some(arguments_node),
                ctx(ARGUMENTS, node),
                Self::translate_code_template_element,
            );
        }
        Some(())
    }

    /// `_translateReplacedByChangeLibrary`.
    fn translate_replaced_by_change_library(
        &mut self,
        node: &YamlNode,
        _context: &Ctx,
    ) -> Option<()> {
        if !is_map(node) {
            return None;
        }
        self.report_unsupported_keys(node, &[KIND_KEY, NEW_LIBRARY_KEY]);
        let change_key =
            self.translate_string(node.value_at(KIND_KEY), ctx(KIND_KEY, node), true)?;
        if change_key != REPLACED_BY_KIND {
            let span = node.value_at(KIND_KEY)?.span;
            self.report(
                diag::invalid_change_for_kind(
                    REPLACED_BY_KIND,
                    ElementKind::Library.display_name(),
                ),
                span,
            );
            return None;
        }
        self.translate_library(node.value_at(NEW_LIBRARY_KEY), ctx(NEW_LIBRARY_KEY, node))?;
        Some(())
    }

    /// `_translateConditionalChange`.
    fn translate_conditional_change(&mut self, node: &YamlNode, context: Ctx) {
        if !is_map(node) {
            self.report_invalid_value(node, context, "Map");
            return;
        }
        self.report_unsupported_keys(node, &[IF_KEY, CHANGES_KEY]);
        let expression_node = node.value_at(IF_KEY);
        let expression_text = self.translate_string(expression_node, ctx(IF_KEY, node), true);
        let changes = self.translate_list(
            node.value_at(CHANGES_KEY),
            ctx(CHANGES_KEY, node),
            Self::translate_change,
        );
        if let (Some(expression_node), Some(text), Some(_)) =
            (expression_node, expression_text, changes)
            && matches!(expression_node.kind, NodeKind::Scalar(_))
        {
            let scope = self.transform_scope.clone();
            let _ = self.parse_condition(&text, offset_of_string(expression_node), &scope);
        }
    }

    /// `_translateConditionalChanges`.
    fn translate_conditional_changes(&mut self, node: &YamlNode, context: Ctx) -> Option<()> {
        if let NodeKind::List(elements) = &node.kind {
            for element in elements {
                self.translate_conditional_change(element, context);
            }
            Some(())
        } else {
            self.report_invalid_value(node, context, "List");
            None
        }
    }

    // ---- elements ------------------------------------------------------------

    /// `_translateElement`.
    fn translate_element(&mut self, node: Option<&YamlNode>, context: Ctx) -> Option<ElementKind> {
        let node = match node {
            Some(node) if is_map(node) => node,
            None => {
                self.report_missing_key(context);
                return None;
            }
            Some(node) => {
                self.report_invalid_value(node, context, "Map");
                return None;
            }
        };
        let uris_node = node.value_at(URIS_KEY);
        let uris = self.translate_list(uris_node, ctx(URIS_KEY, node), Self::translate_uri_element);
        let (element_key, element_value) =
            self.single_key(node, &[&ELEMENT_KEYS[..]], node.span, true)?;
        let element_name =
            self.translate_string(Some(element_value), ctx(&element_key, node), true)?;
        let _ = element_name;
        let mut component_count = 1;
        let static_node = node.value_at(STATIC_KEY);
        if let Some(valid_container_keys) = container_keys(&element_key) {
            let container = self.single_key(node, &[valid_container_keys], node.span, false);
            let container_name = match container {
                Some((key, value)) => self.translate_string(Some(value), ctx(&key, node), false),
                None => None,
            };
            match container_name {
                None => {
                    if [CONSTRUCTOR_KEY, CONSTANT_KEY, METHOD_KEY, FIELD_KEY]
                        .contains(&element_key.as_str())
                    {
                        self.report(
                            diag::missing_one_of_multiple_keys(&quoted_with_or(
                                valid_container_keys,
                            )),
                            node.span,
                        );
                        return None;
                    }
                }
                Some(_) => component_count += 1,
            }
            if let Some(static_node) = static_node
                && self
                    .translate_bool(Some(static_node), ctx(STATIC_KEY, node), true)
                    .is_some()
                && component_count == 1
                && let Some(key) = get_key(node, STATIC_KEY)
            {
                self.report(diag::unsupported_static(), key.span);
            }
        } else if static_node.is_some()
            && let Some(key) = get_key(node, STATIC_KEY)
        {
            self.report(diag::unsupported_static(), key.span);
        }
        let uris = uris?;
        if uris.is_empty() {
            if let Some(uris_node) = uris_node
                && matches!(&uris_node.kind, NodeKind::List(list) if list.is_empty())
            {
                self.report(diag::missing_uri(), uris_node.span);
            }
            return None;
        }
        ElementKind::from_name(&element_key)
    }

    /// `_translateLibrary`.
    fn translate_library(&mut self, node: Option<&YamlNode>, context: Ctx) -> Option<()> {
        match node {
            Some(node) => {
                if let NodeKind::Scalar(value) = &node.kind {
                    if !matches!(value, Scalar::String(_)) {
                        self.report_invalid_value(node, context, "String");
                        return None;
                    }
                    return self.translate_uri(Some(node), context, true);
                }
                self.report_invalid_value(node, context, "ElementDescriptor");
                None
            }
            None => {
                self.report_missing_key(context);
                None
            }
        }
    }

    // ---- transforms ------------------------------------------------------------

    /// `_translateElementTransform`.
    fn translate_element_transform(&mut self, node: &YamlNode, context: Ctx) -> Option<()> {
        let element = self.translate_element(node.value_at(ELEMENT_KEY), ctx(ELEMENT_KEY, node));
        self.element_being_transformed = element;
        self.transform_scope = self
            .translate_template_variables(node.value_at(VARIABLES_KEY), ctx(VARIABLES_KEY, node));
        let selector =
            match self.single_key(node, &[&[CHANGES_KEY], &[ONE_OF_KEY]], context.parent, true) {
                Some((key, value)) if key == CHANGES_KEY => self
                    .translate_list(Some(value), ctx(CHANGES_KEY, node), Self::translate_change)
                    .map(|_| ()),
                Some((_, value)) => {
                    self.translate_conditional_changes(value, ctx(ONE_OF_KEY, node))
                }
                None => None,
            };
        self.transform_scope = Scope::new();
        if element.is_none() || selector.is_none() {
            return None;
        }
        Some(())
    }

    /// `_translateLibraryTransform`.
    fn translate_library_transform(&mut self, node: &YamlNode) -> Option<()> {
        self.translate_library(node.value_at(LIBRARY_KEY), ctx(LIBRARY_KEY, node))?;
        self.element_being_transformed = Some(ElementKind::Library);
        let Some(changes_node) = node.value_at(CHANGES_KEY) else {
            self.report(diag::missing_key(CHANGES_KEY), node.span);
            return None;
        };
        let changes = self.translate_list(
            Some(changes_node),
            ctx(CHANGES_KEY, node),
            Self::translate_replaced_by_change_library,
        )?;
        if changes.len() > 1 {
            self.report(
                diag::invalid_change_for_kind(
                    REPLACED_BY_KIND,
                    ElementKind::Library.display_name(),
                ),
                changes_node.span,
            );
            return None;
        }
        Some(())
    }

    /// `_translateTransform`.
    fn translate_transform(&mut self, node: &YamlNode, context: &Ctx) -> Option<()> {
        if !is_map(node) {
            self.report_invalid_value(node, *context, "Map");
            return None;
        }
        self.report_unsupported_keys(
            node,
            &[
                BULK_APPLY_KEY,
                CHANGES_KEY,
                DATE_KEY,
                ELEMENT_KEY,
                LIBRARY_KEY,
                ONE_OF_KEY,
                TITLE_KEY,
                VARIABLES_KEY,
            ],
        );
        let title = self.translate_string(node.value_at(TITLE_KEY), ctx(TITLE_KEY, node), true);
        let date = self.translate_date(node.value_at(DATE_KEY), ctx(DATE_KEY, node));
        let _ = self.translate_bool(
            node.value_at(BULK_APPLY_KEY),
            ctx(BULK_APPLY_KEY, node),
            false,
        );
        if title.is_none() || date.is_none() {
            return None;
        }
        let element_node = node.value_at(ELEMENT_KEY);
        let library_node = node.value_at(LIBRARY_KEY);
        if library_node.is_none() && element_node.is_none() {
            self.report(
                diag::missing_one_of_multiple_keys(&quoted_with_or(&[ELEMENT_KEY, LIBRARY_KEY])),
                node.span,
            );
            return None;
        }
        if library_node.is_some() && element_node.is_some() {
            self.report(diag::conflicting_key(ELEMENT_KEY, LIBRARY_KEY), node.span);
            return None;
        }
        if element_node.is_some() {
            self.translate_element_transform(node, *context)
        } else {
            self.translate_library_transform(node)
        }
    }

    /// `_translateTransformSet`.
    fn translate_transform_set(&mut self, node: &YamlNode) {
        match &node.kind {
            NodeKind::Map(_) => {
                self.report_unsupported_keys(node, &[TRANSFORMS_KEY, VERSION_KEY]);
                let version_node = node.value_at(VERSION_KEY);
                let version = self.translate_integer(version_node, ctx(VERSION_KEY, node));
                let (Some(version_node), Some(version)) = (version_node, version) else {
                    return;
                };
                if !(1..=CURRENT_VERSION).contains(&version) {
                    self.report(diag::unsupported_version(), version_node.span);
                    return;
                }
                let _ = self.translate_list(
                    node.value_at(TRANSFORMS_KEY),
                    ctx(TRANSFORMS_KEY, node),
                    Self::translate_transform,
                );
            }
            NodeKind::Scalar(Scalar::Null) => {}
            _ => {
                let diagnostic = diag::invalid_value("file", "Map", node_type(node));
                self.report(diagnostic, node.span);
            }
        }
    }

    // ---- code fragments (`code_fragment_parser.dart`) --------------------------------

    /// `CodeFragmentParser.parseAccessors`: `Some` when the Dart method
    /// returns a list (possibly empty), `None` when it returns `null`.
    fn parse_accessors(&mut self, content: &str, delta: usize) -> Option<()> {
        let mut tokens = self.scan_fragment(content, delta)?;
        tokens.index = 0;
        if self.parse_accessor(&mut tokens, delta).is_none() {
            return Some(());
        }
        while tokens.index < tokens.tokens.len() {
            let token = tokens.tokens[tokens.index].clone();
            if token.kind == TokenKind::Period {
                tokens.advance();
                if self.parse_accessor(&mut tokens, delta).is_none() {
                    return Some(());
                }
            } else {
                self.report_at(
                    diag::wrong_token(".", token.kind.display_name()),
                    token.offset + delta,
                    token.length(),
                );
                return None;
            }
        }
        Some(())
    }

    /// `CodeFragmentParser.parseCondition`: `Some` for a non-null expression.
    fn parse_condition(&mut self, content: &str, delta: usize, scope: &Scope) -> Option<()> {
        let mut tokens = self.scan_fragment(content, delta)?;
        tokens.index = 0;
        let expression = self.parse_logical_and_expression(&mut tokens, delta, scope);
        if self.aborted {
            return None;
        }
        if tokens.index < tokens.tokens.len() {
            let token = tokens.tokens[tokens.index].clone();
            self.report_at(
                diag::unexpected_transform_set_token(token.kind.display_name()),
                token.offset + delta,
                token.length(),
            );
            return None;
        }
        expression
    }

    /// `_CodeFragmentScanner.scan`.
    fn scan_fragment(&mut self, content: &str, delta: usize) -> Option<TokenStream> {
        let content: Vec<u16> = content.encode_utf16().collect();
        let length = content.len();
        let is_whitespace = |c: u16| c == 0x20 || c == 0x0A || c == 0x0D;
        let skip_whitespace = |mut offset: usize| {
            while offset < length && is_whitespace(content[offset]) {
                offset += 1;
            }
            offset
        };
        let text = |from: usize, to: usize| String::from_utf16_lossy(&content[from..to]);
        let mut tokens = Vec::new();
        let mut offset = skip_whitespace(0);
        while offset < length {
            let c = content[offset];
            let invalid_character = |parser: &mut Self| {
                parser.report_at(
                    diag::invalid_character(&text(offset, offset + 1)),
                    offset + delta,
                    1,
                );
            };
            // `peekAt(offset + 1)`: `RangeError` at the end of the content.
            let peek_next = |parser: &mut Self| -> Option<u16> {
                if offset + 1 >= length {
                    parser.aborted = true;
                    None
                } else {
                    Some(content[offset + 1])
                }
            };
            match c {
                0x5D => {
                    tokens.push(Token::new(offset, TokenKind::CloseSquareBracket, "]"));
                    offset += 1;
                }
                0x5B => {
                    tokens.push(Token::new(offset, TokenKind::OpenSquareBracket, "["));
                    offset += 1;
                }
                0x2E => {
                    tokens.push(Token::new(offset, TokenKind::Period, "."));
                    offset += 1;
                }
                0x26 => {
                    if peek_next(self)? != 0x26 {
                        invalid_character(self);
                        return None;
                    }
                    tokens.push(Token::new(offset, TokenKind::And, "&&"));
                    offset += 2;
                }
                0x21 => {
                    if peek_next(self)? != 0x3D {
                        invalid_character(self);
                        return None;
                    }
                    tokens.push(Token::new(offset, TokenKind::NotEqual, "!="));
                    offset += 2;
                }
                0x3D => {
                    if peek_next(self)? != 0x3D {
                        invalid_character(self);
                        return None;
                    }
                    tokens.push(Token::new(offset, TokenKind::Equal, "=="));
                    offset += 2;
                }
                0x27 => {
                    let start = offset;
                    offset += 1;
                    while offset < length && content[offset] != 0x27 {
                        offset += 1;
                    }
                    offset += 1;
                    if offset > length {
                        // `substring` throws a `RangeError`.
                        self.aborted = true;
                        return None;
                    }
                    tokens.push(Token::new(start, TokenKind::String, &text(start, offset)));
                }
                c if is_letter(c) => {
                    let start = offset;
                    offset += 1;
                    while offset < length && is_letter(content[offset]) {
                        offset += 1;
                    }
                    tokens.push(Token::new(
                        start,
                        TokenKind::Identifier,
                        &text(start, offset),
                    ));
                }
                c if is_digit(c) => {
                    let start = offset;
                    offset += 1;
                    while offset < length && is_digit(content[offset]) {
                        offset += 1;
                    }
                    tokens.push(Token::new(start, TokenKind::Integer, &text(start, offset)));
                }
                _ => {
                    invalid_character(self);
                    return None;
                }
            }
            offset = skip_whitespace(offset);
        }
        Some(TokenStream { tokens, index: 0 })
    }

    /// `_expect`.
    fn expect(&mut self, stream: &TokenStream, valid: &[TokenKind], delta: usize) -> Option<Token> {
        let valid_display = || {
            let names: Vec<&str> = valid.iter().map(|k| k.display_name()).collect();
            let mut out = String::new();
            for (i, name) in names.iter().enumerate() {
                if i > 0 {
                    out.push_str(if i == names.len() - 1 { " or " } else { ", " });
                }
                out.push_str(name);
            }
            out
        };
        let Some(token) = stream.current() else {
            let (offset, length) = match stream.tokens.last() {
                Some(last) => (last.offset, last.length()),
                None => (0, 0),
            };
            self.report_at(
                diag::missing_token(&valid_display()),
                offset + delta,
                length,
            );
            return None;
        };
        if !valid.contains(&token.kind) {
            self.report_at(
                diag::wrong_token(&valid_display(), token.kind.display_name()),
                token.offset + delta,
                token.length(),
            );
            return None;
        }
        Some(token.clone())
    }

    /// `int.parse` of an integer token: a `FormatException` aborts.
    fn parse_int(&mut self, lexeme: &str) -> Option<i64> {
        match lexeme.parse::<i64>() {
            Ok(value) => Some(value),
            Err(_) => {
                self.aborted = true;
                None
            }
        }
    }

    /// `_parseAccessor`.
    fn parse_accessor(&mut self, stream: &mut TokenStream, delta: usize) -> Option<()> {
        let token = self.expect(stream, &[TokenKind::Identifier], delta)?;
        match token.lexeme.as_str() {
            "arguments" => {
                stream.advance();
                self.expect(stream, &[TokenKind::OpenSquareBracket], delta)?;
                stream.advance();
                let token =
                    self.expect(stream, &[TokenKind::Identifier, TokenKind::Integer], delta)?;
                if token.kind == TokenKind::Integer {
                    self.parse_int(&token.lexeme)?;
                }
                stream.advance();
                self.expect(stream, &[TokenKind::CloseSquareBracket], delta)?;
                stream.advance();
                Some(())
            }
            "typeArguments" => {
                stream.advance();
                self.expect(stream, &[TokenKind::OpenSquareBracket], delta)?;
                stream.advance();
                let token = self.expect(stream, &[TokenKind::Integer], delta)?;
                stream.advance();
                self.parse_int(&token.lexeme)?;
                self.expect(stream, &[TokenKind::CloseSquareBracket], delta)?;
                stream.advance();
                Some(())
            }
            other => {
                self.report_at(
                    diag::unknown_accessor(other),
                    token.offset + delta,
                    token.length(),
                );
                None
            }
        }
    }

    /// `_parseEqualityExpression`.
    fn parse_equality_expression(
        &mut self,
        stream: &mut TokenStream,
        delta: usize,
        scope: &Scope,
    ) -> Option<()> {
        self.parse_primary_expression(stream, delta, scope)?;
        if stream.index >= stream.tokens.len() {
            return Some(());
        }
        let kind = stream.current().map(|t| t.kind);
        if kind == Some(TokenKind::Equal) || kind == Some(TokenKind::NotEqual) {
            stream.advance();
            self.parse_primary_expression(stream, delta, scope)?;
        }
        Some(())
    }

    /// `_parseLogicalAndExpression`.
    fn parse_logical_and_expression(
        &mut self,
        stream: &mut TokenStream,
        delta: usize,
        scope: &Scope,
    ) -> Option<()> {
        self.parse_equality_expression(stream, delta, scope)?;
        if stream.index >= stream.tokens.len() {
            return Some(());
        }
        let mut kind = stream.current().map(|t| t.kind);
        while kind == Some(TokenKind::And) {
            stream.advance();
            self.parse_equality_expression(stream, delta, scope)?;
            if stream.index >= stream.tokens.len() {
                return Some(());
            }
            kind = stream.current().map(|t| t.kind);
        }
        Some(())
    }

    /// `_parsePrimaryExpression`.
    fn parse_primary_expression(
        &mut self,
        stream: &mut TokenStream,
        delta: usize,
        scope: &Scope,
    ) -> Option<()> {
        let token = stream.current().cloned();
        if let Some(token) = &token {
            match token.kind {
                TokenKind::Identifier => {
                    stream.advance();
                    if !scope.contains(&token.lexeme) {
                        self.report_at(
                            diag::undefined_variable(&token.lexeme),
                            token.offset + delta,
                            token.length(),
                        );
                        return None;
                    }
                    return Some(());
                }
                TokenKind::String => {
                    stream.advance();
                    return Some(());
                }
                _ => {}
            }
        }
        let (offset, length) = match &token {
            None => match stream.tokens.last() {
                Some(last) => (last.offset + delta, last.length()),
                None => (delta, 0),
            },
            Some(token) => (token.offset + delta, token.length()),
        };
        self.report_at(diag::expected_primary(), offset, length);
        None
    }
}

fn is_letter(c: u16) -> bool {
    (0x41..=0x5A).contains(&c) || (0x61..=0x7A).contains(&c)
}

fn is_digit(c: u16) -> bool {
    (0x30..=0x39).contains(&c)
}

/// `String.indexOf(pattern, start)` on UTF-16 code units.
fn index_of(text: &[u16], pattern: &[u16], start: usize) -> Option<usize> {
    if start > text.len() {
        return None;
    }
    if pattern.is_empty() {
        return Some(start);
    }
    (start..=text.len().checked_sub(pattern.len())?)
        .find(|&i| &text[i..i + pattern.len()] == pattern)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TokenKind {
    And,
    CloseSquareBracket,
    Equal,
    Identifier,
    Integer,
    NotEqual,
    OpenSquareBracket,
    Period,
    String,
}

impl TokenKind {
    fn display_name(self) -> &'static str {
        match self {
            TokenKind::And => "'&&'",
            TokenKind::CloseSquareBracket => "']'",
            TokenKind::Equal => "'=='",
            TokenKind::Identifier => "an identifier",
            TokenKind::Integer => "an integer",
            TokenKind::NotEqual => "'!='",
            TokenKind::OpenSquareBracket => "'['",
            TokenKind::Period => "'.'",
            TokenKind::String => "a string",
        }
    }
}

#[derive(Clone, Debug)]
struct Token {
    offset: usize,
    kind: TokenKind,
    lexeme: String,
    /// Length in UTF-16 code units.
    length: usize,
}

impl Token {
    fn new(offset: usize, kind: TokenKind, lexeme: &str) -> Token {
        Token {
            offset,
            kind,
            lexeme: lexeme.to_string(),
            length: lexeme.encode_utf16().count(),
        }
    }

    fn length(&self) -> usize {
        self.length
    }
}

struct TokenStream {
    tokens: Vec<Token>,
    index: usize,
}

impl TokenStream {
    fn current(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }

    fn advance(&mut self) {
        if self.index < self.tokens.len() {
            self.index += 1;
        }
    }
}

/// Whether `DateTime.parse` accepts [value] (`DateTime._parse`).
///
/// The range check treats local times like UTC times: the pinned Dart result
/// depends on the time zone of the machine only within a day of the limits.
fn date_time_parses(value: &str) -> bool {
    let bytes = value.as_bytes();
    let digits = |from: usize, count: usize| -> Option<i64> {
        let slice = bytes.get(from..from + count)?;
        if slice.iter().all(u8::is_ascii_digit) {
            std::str::from_utf8(slice).ok()?.parse().ok()
        } else {
            None
        }
    };
    // `([+-]?\d{4,6})-?(\d\d)-?(\d\d)`; the regular expression backtracks
    // over the number of year digits.
    let (sign, year_start) = match bytes.first() {
        Some(b'+') => (1, 1),
        Some(b'-') => (-1, 1),
        _ => (1, 0),
    };
    let mut parsed = None;
    for year_digits in (4..=6).rev() {
        let Some(year) = digits(year_start, year_digits) else {
            continue;
        };
        let mut at = year_start + year_digits;
        if bytes.get(at) == Some(&b'-') {
            at += 1;
        }
        let Some(month) = digits(at, 2) else { continue };
        at += 2;
        if bytes.get(at) == Some(&b'-') {
            at += 1;
        }
        let Some(day) = digits(at, 2) else { continue };
        at += 2;
        if let Some(rest) = parse_time_part(bytes, at) {
            parsed = Some((sign * year, month, day, rest));
            break;
        }
    }
    let Some((year, month, day, (hour, minute, second, millisecond, offset_minutes))) = parsed
    else {
        return false;
    };
    // Days since the epoch with month and day overflow (`_brokenDownDateToValue`).
    let month_index = month - 1;
    let year = year + month_index.div_euclid(12);
    let month = month_index.rem_euclid(12) + 1;
    let days = days_from_civil(year, month, 1) + (day - 1);
    let milliseconds =
        (((days * 24 + hour) * 60 + minute - offset_minutes) * 60 + second) * 1000 + millisecond;
    milliseconds.abs() <= 8_640_000_000_000_000
}

/// The optional time and time zone part of the `DateTime.parse` pattern.
/// Returns `(hour, minute, second, millisecond, utc offset in minutes)` when
/// the rest of the string matches up to the end.
fn parse_time_part(bytes: &[u8], at: usize) -> Option<(i64, i64, i64, i64, i64)> {
    if at == bytes.len() {
        return Some((0, 0, 0, 0, 0));
    }
    let digits = |from: usize, count: usize| -> Option<i64> {
        let slice = bytes.get(from..from + count)?;
        if slice.iter().all(u8::is_ascii_digit) {
            std::str::from_utf8(slice).ok()?.parse().ok()
        } else {
            None
        }
    };
    if !matches!(bytes.get(at), Some(b' ') | Some(b'T')) {
        return None;
    }
    let mut at = at + 1;
    let hour = digits(at, 2)?;
    at += 2;
    let (mut minute, mut second, mut millisecond) = (0, 0, 0);
    // `(?::?(\d\d)(?::?(\d\d)(?:[.,](\d+))?)?)?`
    let mut look = at;
    if bytes.get(look) == Some(&b':') {
        look += 1;
    }
    if let Some(value) = digits(look, 2) {
        minute = value;
        at = look + 2;
        let mut look = at;
        if bytes.get(look) == Some(&b':') {
            look += 1;
        }
        if let Some(value) = digits(look, 2) {
            second = value;
            at = look + 2;
            if matches!(bytes.get(at), Some(b'.') | Some(b',')) {
                let start = at + 1;
                let mut end = start;
                while bytes.get(end).is_some_and(u8::is_ascii_digit) {
                    end += 1;
                }
                if end > start {
                    let fraction = &bytes[start..end.min(start + 3)];
                    let mut value: i64 = std::str::from_utf8(fraction).ok()?.parse().ok()?;
                    for _ in fraction.len()..3 {
                        value *= 10;
                    }
                    millisecond = value;
                    at = end;
                }
            }
        }
    }
    // `( ?[zZ]| ?([-+])(\d\d)(?::?(\d\d))?)?$`
    let mut offset_minutes = 0;
    if at < bytes.len() {
        let mut look = at;
        if bytes.get(look) == Some(&b' ') {
            look += 1;
        }
        match bytes.get(look) {
            Some(b'z') | Some(b'Z') => look += 1,
            Some(&sign @ (b'+' | b'-')) => {
                let hours = digits(look + 1, 2)?;
                look += 3;
                let mut minutes = 0;
                let mut after = look;
                if bytes.get(after) == Some(&b':') {
                    after += 1;
                }
                if let Some(value) = digits(after, 2) {
                    minutes = value;
                    look = after + 2;
                }
                offset_minutes = (hours * 60 + minutes) * if sign == b'-' { -1 } else { 1 };
            }
            _ => return None,
        }
        if look != bytes.len() {
            return None;
        }
    }
    Some((hour, minute, second, millisecond, offset_minutes))
}

/// Days since 1970-01-01 of a proleptic Gregorian date.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// Whether `Uri.parse` accepts [value]. An approximation of the cases in
/// which the pinned implementation throws a `FormatException`: an invalid
/// scheme, a port that is not a number, and an invalid bracketed host.
fn uri_parses(value: &str) -> bool {
    let end_of_scheme = value.find([':', '/', '?', '#']);
    let mut rest = value;
    if let Some(colon) = end_of_scheme.filter(|&i| value.as_bytes()[i] == b':') {
        let scheme = &value[..colon];
        let mut chars = scheme.chars();
        let valid = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
            && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
        if !valid {
            return false;
        }
        rest = &value[colon + 1..];
    }
    let Some(after_slashes) = rest.strip_prefix("//") else {
        return true;
    };
    let authority_end = after_slashes
        .find(['/', '?', '#'])
        .unwrap_or(after_slashes.len());
    let authority = &after_slashes[..authority_end];
    let host_and_port = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let (host, port) = if let Some(inner) = host_and_port.strip_prefix('[') {
        let Some(close) = inner.find(']') else {
            return false;
        };
        let address = &inner[..close];
        let after = &inner[close + 1..];
        let address_ok = address.starts_with('v')
            || address.starts_with('V')
            || address
                .split_once('%')
                .map_or(address, |(a, _)| a)
                .parse::<std::net::Ipv6Addr>()
                .is_ok();
        if !address_ok {
            return false;
        }
        match after {
            "" => ("", None),
            _ => match after.strip_prefix(':') {
                Some(port) => ("", Some(port)),
                None => return false,
            },
        }
    } else {
        match host_and_port.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (host_and_port, None),
        }
    };
    if host.contains(['[', ']']) {
        return false;
    }
    match port {
        Some(port) => {
            let digits = port.strip_prefix(['+', '-']).unwrap_or(port);
            digits.chars().all(|c| c.is_ascii_digit())
        }
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn messages(text: &str) -> Vec<String> {
        validate_fix_data(text, "p")
            .into_iter()
            .map(|d| format!("{}@{}+{}: {}", d.code.name, d.offset, d.length, d.message))
            .collect()
    }

    #[test]
    fn valid_file_has_no_diagnostics() {
        let text = "version: 1\ntransforms:\n  - title: 'Rename'\n    date: 2020-01-01\n    element:\n      uris: ['a.dart']\n      method: 'f'\n      inClass: 'C'\n    changes:\n      - kind: 'rename'\n        newName: 'g'\n";
        assert_eq!(messages(text), Vec::<String>::new());
    }

    #[test]
    fn missing_version_and_bad_file_type() {
        assert_eq!(messages("transforms: []\n").len(), 1);
        assert_eq!(
            messages("- a\n"),
            vec![
                "invalid_value@0+3: The value of 'file' should be of type 'Map' but is of type 'List'."
            ]
        );
        assert!(messages("# only a comment\n").is_empty());
    }

    #[test]
    fn code_fragment_exceptions_drop_all_diagnostics() {
        // `'` without the closing quote: `substring` throws in Dart.
        let text = "version: 1\ntransforms:\n  - title: t\n    date: 2020-01-01\n    bogus: 1\n    element:\n      uris: ['a.dart']\n      function: f\n    variables:\n      v:\n        kind: fragment\n        value: \"arguments[0]\"\n    oneOf:\n      - if: \"v == 'x\"\n        changes: []\n";
        assert!(messages(text).is_empty());
    }

    #[test]
    fn date_and_uri_parsing() {
        assert!(date_time_parses("2020-01-01"));
        assert!(date_time_parses("20200101"));
        assert!(date_time_parses("2020-13-45"));
        assert!(date_time_parses("2020-01-01T10:00:00.123456789Z"));
        assert!(date_time_parses("2020-01-01T10:00:00 +0530"));
        assert!(!date_time_parses("2020-1-1"));
        assert!(!date_time_parses("2020-01-01Z"));
        assert!(!date_time_parses("275761-01-01"));
        assert!(date_time_parses("275760-09-13"));
        assert!(!date_time_parses(""));
        assert!(uri_parses("package:a/b.dart"));
        assert!(!uri_parses("1abc:x"));
        assert!(!uri_parses("http://h:1a/"));
        assert!(!uri_parses("http://[zz]/"));
        assert!(uri_parses("http://[::1]:8080/"));
    }
}

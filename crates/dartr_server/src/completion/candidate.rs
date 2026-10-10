// Dart source: pkg/analysis_server/lib/src/services/completion/dart/candidate_suggestion.dart
// Dart source: pkg/analysis_server/lib/src/services/completion/dart/utilities.dart (buildClosureParameters)

//! The candidate suggestions of a completion request (Dart
//! `CandidateSuggestion` and its subclasses).

#[allow(unused_imports)]
use dartr_typesystem::TypeExt;
use dartr_element::{Ctx, ElemRef, ElementId, TypeId, TypeKind};
use dartr_resolver::error::support;
use dartr_typesystem::member;

/// Dart `CompletionSuggestionKind` (the two kinds of executables).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SuggestionKind {
    Identifier,
    Invocation,
}

/// Dart `ImportData`.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportData {
    pub library_uri: String,
    pub prefix: Option<String>,
    pub is_not_imported: bool,
}

/// Dart `TypeImportData`.
#[derive(Clone, Debug, Default)]
pub struct TypeImportData {
    pub completion: String,
    pub display_text: String,
    pub imports: Vec<String>,
    pub selection_offset: Option<usize>,
    pub selection_length: Option<usize>,
}

/// The fields of the typed suggestions (Dart `TypedSuggestion`).
#[derive(Clone, Debug, Default)]
pub struct Typed {
    pub add_type_annotation: bool,
    /// Dart `keyword` (`final` or `var`).
    pub keyword: Option<&'static str>,
    pub add_type_name: bool,
    pub replacement: (u32, u32),
    pub data: Option<TypeImportData>,
}

/// One kind of candidate.
#[derive(Clone, Debug)]
pub enum Kind {
    Class(ElementId),
    Closure {
        function_type: TypeId,
        include_trailing_comma: bool,
        use_block_statement: bool,
        include_types: bool,
        indent: String,
        end_of_line: String,
    },
    Constructor {
        element: ElemRef,
        alias: Option<ElementId>,
        has_class_name: bool,
        is_tear_off: bool,
        is_redirect: bool,
        suggest_unnamed_as_new: bool,
        kind: SuggestionKind,
        typed: Typed,
    },
    EnumConstant {
        element: ElementId,
        include_enum_name: bool,
    },
    Enum(ElementId),
    Extension {
        element: ElementId,
        kind: SuggestionKind,
    },
    ExtensionType(ElementId),
    Field {
        element: ElemRef,
        referencing_interface: Option<ElementId>,
        is_in_declaration: bool,
        typed: Typed,
    },
    FormalParameter {
        element: ElementId,
        distance: i32,
    },
    FunctionCall {
        ty: TypeId,
        element: Option<ElemRef>,
        kind: SuggestionKind,
        typed: Typed,
    },
    Getter {
        element: ElemRef,
        referencing_interface: Option<ElementId>,
        with_enclosing_name: bool,
        typed: Typed,
    },
    Identifier {
        identifier: String,
        include_body: bool,
    },
    ImportPrefix {
        library: ElementId,
        prefix: ElementId,
    },
    Keyword {
        completion: String,
        selection_offset: usize,
    },
    Label(String),
    LoadLibrary {
        element: ElementId,
    },
    LocalFunction {
        element: ElementId,
        kind: SuggestionKind,
    },
    LocalVariable {
        element: ElementId,
        distance: i32,
    },
    Method {
        element: ElemRef,
        kind: SuggestionKind,
        referencing_interface: Option<ElementId>,
        typed: Typed,
    },
    Mixin(ElementId),
    NamedArgument {
        parameter: ElemRef,
        append_colon: bool,
        append_comma: bool,
        replacement_length: Option<u32>,
        is_widget: bool,
        quote: char,
    },
    Name(String),
    Override {
        element: ElemRef,
        should_invoke_super: bool,
        skip_at: bool,
        replacement: (u32, u32),
        data: Option<TypeImportData>,
    },
    RecordField {
        field_type: TypeId,
        name: String,
        typed: Typed,
    },
    RecordLiteralNamedField {
        name: String,
        field_type: TypeId,
        append_colon: bool,
        append_comma: bool,
    },
    SetState {
        element: ElemRef,
        referencing_interface: Option<ElementId>,
        indent: String,
        end_of_line: String,
        kind: SuggestionKind,
        typed: Typed,
    },
    Setter {
        element: ElemRef,
        referencing_interface: Option<ElementId>,
        with_enclosing_name: bool,
    },
    StaticField(ElementId),
    SuperParameter(ElemRef),
    TopLevelFunction {
        element: ElementId,
        kind: SuggestionKind,
    },
    TopLevelGetter(ElementId),
    TopLevelSetter(ElementId),
    TopLevelVariable(ElementId),
    TypeAlias(ElementId),
    TypeParameter(ElementId),
    Uri(String),
}

/// Dart `CandidateSuggestion`.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub kind: Kind,
    /// Dart `ImportableSuggestion.importData`.
    pub import_data: Option<ImportData>,
    pub matcher_score: f64,
    pub relevance: i32,
}

/// Dart `SuggestionData._Data`.
pub struct Data {
    pub completion: String,
    pub selection_offset: usize,
    pub display_text: String,
}

impl Candidate {
    pub fn new(kind: Kind, matcher_score: f64) -> Candidate {
        Candidate {
            kind,
            import_data: None,
            matcher_score,
            relevance: -1,
        }
    }

    pub fn with_import(kind: Kind, import_data: Option<ImportData>, matcher_score: f64) -> Self {
        Candidate {
            kind,
            import_data,
            matcher_score,
            relevance: -1,
        }
    }

    /// Dart `ImportableSuggestion.isNotImported`.
    pub fn is_not_imported(&self) -> bool {
        self.import_data.as_ref().is_some_and(|d| d.is_not_imported)
    }

    /// Dart `ImportableSuggestion.completionPrefix`.
    fn completion_prefix(&self) -> String {
        match self.import_data.as_ref().and_then(|d| d.prefix.as_deref()) {
            Some(p) => format!("{p}."),
            None => String::new(),
        }
    }

    /// Dart `ElementBasedSuggestion.element`.
    pub fn element(&self) -> Option<ElemRef> {
        Some(match &self.kind {
            Kind::Class(e)
            | Kind::Enum(e)
            | Kind::ExtensionType(e)
            | Kind::Mixin(e)
            | Kind::StaticField(e)
            | Kind::TopLevelGetter(e)
            | Kind::TopLevelSetter(e)
            | Kind::TopLevelVariable(e)
            | Kind::TypeAlias(e)
            | Kind::TypeParameter(e) => ElemRef::Base(*e),
            Kind::Constructor { element, .. }
            | Kind::Field { element, .. }
            | Kind::Getter { element, .. }
            | Kind::Method { element, .. }
            | Kind::SetState { element, .. }
            | Kind::Setter { element, .. }
            | Kind::Override { element, .. } => *element,
            Kind::EnumConstant { element, .. }
            | Kind::Extension { element, .. }
            | Kind::FormalParameter { element, .. }
            | Kind::LoadLibrary { element }
            | Kind::LocalFunction { element, .. }
            | Kind::LocalVariable { element, .. }
            | Kind::TopLevelFunction { element, .. } => ElemRef::Base(*element),
            Kind::SuperParameter(e) => *e,
            Kind::ImportPrefix { prefix, .. } => ElemRef::Base(*prefix),
            _ => return None,
        })
    }

    /// Whether this is a Dart `ImportableSuggestion`.
    pub fn is_importable(&self) -> bool {
        matches!(
            self.kind,
            Kind::Class(_)
                | Kind::Constructor { .. }
                | Kind::EnumConstant { .. }
                | Kind::Enum(_)
                | Kind::Extension { .. }
                | Kind::ExtensionType(_)
                | Kind::FunctionCall { .. }
                | Kind::Getter { .. }
                | Kind::LoadLibrary { .. }
                | Kind::LocalFunction { .. }
                | Kind::Method { .. }
                | Kind::Mixin(_)
                | Kind::SetState { .. }
                | Kind::Setter { .. }
                | Kind::StaticField(_)
                | Kind::TopLevelFunction { .. }
                | Kind::TopLevelGetter(_)
                | Kind::TopLevelSetter(_)
                | Kind::TopLevelVariable(_)
                | Kind::TypeAlias(_)
        )
    }

    /// The executable kind (Dart `ExecutableSuggestion.kind`).
    pub fn executable_kind(&self) -> Option<SuggestionKind> {
        match &self.kind {
            Kind::Constructor { kind, .. }
            | Kind::Extension { kind, .. }
            | Kind::FunctionCall { kind, .. }
            | Kind::LocalFunction { kind, .. }
            | Kind::Method { kind, .. }
            | Kind::SetState { kind, .. }
            | Kind::TopLevelFunction { kind, .. } => Some(*kind),
            Kind::LoadLibrary { .. } => Some(SuggestionKind::Invocation),
            _ => None,
        }
    }

    /// The typed part of a Dart `TypedSuggestion`.
    pub fn typed(&self) -> Option<&Typed> {
        match &self.kind {
            Kind::Constructor { typed, .. }
            | Kind::Field { typed, .. }
            | Kind::FunctionCall { typed, .. }
            | Kind::Getter { typed, .. }
            | Kind::Method { typed, .. }
            | Kind::RecordField { typed, .. }
            | Kind::SetState { typed, .. } => Some(typed),
            _ => None,
        }
    }

    pub fn typed_mut(&mut self) -> Option<&mut Typed> {
        match &mut self.kind {
            Kind::Constructor { typed, .. }
            | Kind::Field { typed, .. }
            | Kind::FunctionCall { typed, .. }
            | Kind::Getter { typed, .. }
            | Kind::Method { typed, .. }
            | Kind::RecordField { typed, .. }
            | Kind::SetState { typed, .. } => Some(typed),
            _ => None,
        }
    }

    /// Dart `TypedSuggestion.baseCompletion`.
    pub fn base_completion(&self, ctx: &Ctx<'_>) -> String {
        match &self.kind {
            Kind::Constructor {
                element,
                alias,
                has_class_name,
                suggest_unnamed_as_new,
                typed,
                ..
            } => {
                let base = member::base_element(ctx, *element);
                let mut completion = ctx.element_name(base).unwrap_or("").to_string();
                if *suggest_unnamed_as_new {
                    if completion.is_empty() {
                        completion = "new".to_string();
                    }
                } else if completion == "new" {
                    completion = String::new();
                }
                if !has_class_name && !typed.add_type_name {
                    let class_name = match alias {
                        Some(a) => support::display_name(ctx, *a),
                        None => ctx
                            .element_data(base)
                            .and_then(|d| d.enclosing)
                            .map(|c| support::display_name(ctx, c))
                            .unwrap_or_default(),
                    };
                    if completion.is_empty() {
                        completion = class_name;
                    } else {
                        completion = format!("{class_name}.{completion}");
                    }
                }
                completion
            }
            Kind::Field {
                element,
                is_in_declaration,
                ..
            } => {
                let base = member::base_element(ctx, *element);
                if is_enum_constant(ctx, base) {
                    let name = ctx.element_name(base).unwrap_or("");
                    if *is_in_declaration {
                        return name.to_string();
                    }
                    let enum_name = ctx
                        .element_data(base)
                        .and_then(|d| d.enclosing)
                        .map(|c| support::display_name(ctx, c))
                        .unwrap_or_default();
                    return format!("{enum_name}.{name}");
                }
                display_name(ctx, base)
            }
            Kind::FunctionCall { .. } => "call".to_string(),
            Kind::Getter {
                element,
                with_enclosing_name,
                ..
            } => {
                let base = member::base_element(ctx, *element);
                let name = display_name(ctx, base);
                if *with_enclosing_name {
                    if let Some(e) = ctx.element_data(base).and_then(|d| d.enclosing) {
                        return format!("{}.{name}", support::display_name(ctx, e));
                    }
                }
                name
            }
            Kind::Method { element, .. } => display_name(ctx, member::base_element(ctx, *element)),
            Kind::RecordField { name, .. } => name.clone(),
            Kind::SetState { .. } => self.suggestion_data(ctx).map(|d| d.completion).unwrap(),
            _ => self.completion(ctx),
        }
    }

    /// Dart `CandidateSuggestion.completion`.
    pub fn completion(&self, ctx: &Ctx<'_>) -> String {
        if let Some(typed) = self.typed() {
            if let Some(data) = &typed.data {
                return data.completion.clone();
            }
            return self.base_completion(ctx);
        }
        match &self.kind {
            Kind::Class(e)
            | Kind::Enum(e)
            | Kind::ExtensionType(e)
            | Kind::Mixin(e)
            | Kind::TopLevelGetter(e)
            | Kind::TopLevelSetter(e)
            | Kind::TopLevelVariable(e)
            | Kind::TypeAlias(e)
            | Kind::Extension { element: e, .. }
            | Kind::TopLevelFunction { element: e, .. } => {
                format!("{}{}", self.completion_prefix(), display_name(ctx, *e))
            }
            Kind::Closure { .. } | Kind::NamedArgument { .. } | Kind::RecordLiteralNamedField { .. } => {
                self.suggestion_data(ctx).map(|d| d.completion).unwrap()
            }
            Kind::EnumConstant {
                element,
                include_enum_name,
            } => {
                if *include_enum_name {
                    let enclosing = ctx
                        .element_data(*element)
                        .and_then(|d| d.enclosing)
                        .map(|c| support::display_name(ctx, c))
                        .unwrap_or_default();
                    format!(
                        "{}{enclosing}.{}",
                        self.completion_prefix(),
                        display_name(ctx, *element)
                    )
                } else {
                    display_name(ctx, *element)
                }
            }
            Kind::FormalParameter { element, .. }
            | Kind::LoadLibrary { element }
            | Kind::LocalFunction { element, .. }
            | Kind::LocalVariable { element, .. }
            | Kind::TypeParameter(element) => display_name(ctx, *element),
            Kind::SuperParameter(e) => display_name(ctx, member::base_element(ctx, *e)),
            Kind::Identifier {
                identifier,
                include_body,
            } => {
                if *include_body {
                    format!("{identifier} {{}}")
                } else {
                    identifier.clone()
                }
            }
            Kind::ImportPrefix { prefix, .. } => display_name(ctx, *prefix),
            Kind::Keyword { completion, .. } => completion.clone(),
            Kind::Label(name) => name.clone(),
            Kind::Name(name) => name.clone(),
            Kind::Override { element, data, .. } => match data {
                Some(d) => d.completion.clone(),
                None => format!(
                    "@override {}",
                    display_name(ctx, member::base_element(ctx, *element))
                ),
            },
            Kind::Setter {
                element,
                with_enclosing_name,
                ..
            } => {
                let base = member::base_element(ctx, *element);
                let name = display_name(ctx, base);
                if *with_enclosing_name {
                    if let Some(e) = ctx.element_data(base).and_then(|d| d.enclosing) {
                        return format!("{}.{name}", support::display_name(ctx, e));
                    }
                }
                name
            }
            Kind::StaticField(e) => {
                let enclosing = ctx
                    .element_data(*e)
                    .and_then(|d| d.enclosing)
                    .map(|c| support::display_name(ctx, c))
                    .unwrap_or_default();
                format!(
                    "{}{enclosing}.{}",
                    self.completion_prefix(),
                    display_name(ctx, *e)
                )
            }
            Kind::Uri(uri) => uri.clone(),
            _ => self.base_completion(ctx),
        }
    }

    /// Dart `SuggestionData` (closures, named arguments, `setState`,
    /// record literal named fields).
    pub fn suggestion_data(&self, ctx: &Ctx<'_>) -> Option<Data> {
        match &self.kind {
            Kind::Closure {
                function_type,
                include_trailing_comma,
                use_block_statement,
                include_types,
                indent,
                end_of_line,
            } => {
                let parameters = build_closure_parameters(ctx, *function_type, *include_types, true);
                let display_parameters = build_closure_parameters(ctx, *function_type, false, false);
                let mut buffer = parameters;
                let display_text;
                let selection_offset;
                if *use_block_statement {
                    display_text = format!("{display_parameters} {{}}");
                    buffer.push_str(" {");
                    buffer.push_str(end_of_line);
                    buffer.push_str(&format!("{indent}  "));
                    selection_offset = utf16_len(&buffer);
                    buffer.push_str(end_of_line);
                    buffer.push_str(&format!("{indent}}}"));
                } else {
                    display_text = format!("{display_parameters} =>");
                    buffer.push_str(" => ");
                    selection_offset = utf16_len(&buffer);
                }
                if *include_trailing_comma {
                    buffer.push(',');
                }
                Some(Data {
                    completion: buffer,
                    selection_offset,
                    display_text,
                })
            }
            Kind::NamedArgument {
                parameter,
                append_colon,
                append_comma,
                is_widget,
                quote,
                ..
            } => {
                let base = member::base_element(ctx, *parameter);
                let mut completion = display_name(ctx, base);
                if *append_colon {
                    completion.push_str(": ");
                }
                let mut selection_offset = utf16_len(&completion);
                if *is_widget && *append_colon {
                    let ty = member::type_(ctx, *parameter);
                    if let Some((text, cursor)) = default_string_parameter_value(ctx, ty, *quote) {
                        if text == "[]" {
                            let length = utf16_len(&completion);
                            completion.push_str(&text);
                            if let Some(c) = cursor {
                                selection_offset = length + c;
                            }
                        }
                    }
                }
                if *append_comma {
                    completion.push(',');
                }
                Some(Data {
                    display_text: completion.clone(),
                    completion,
                    selection_offset,
                })
            }
            Kind::RecordLiteralNamedField {
                name,
                append_colon,
                append_comma,
                ..
            } => {
                let mut completion = name.clone();
                if *append_colon {
                    completion.push_str(": ");
                }
                let selection_offset = utf16_len(&completion);
                if *append_comma {
                    completion.push(',');
                }
                Some(Data {
                    display_text: completion.clone(),
                    completion,
                    selection_offset,
                })
            }
            Kind::SetState {
                indent,
                end_of_line,
                ..
            } => {
                let mut buffer = String::from("setState(() {");
                buffer.push_str(end_of_line);
                buffer.push_str(&format!("{indent}  "));
                let selection_offset = utf16_len(&buffer);
                buffer.push_str(end_of_line);
                buffer.push_str(&format!("{indent}}});"));
                Some(Data {
                    completion: buffer,
                    selection_offset,
                    display_text: "setState(() {});".to_string(),
                })
            }
            _ => None,
        }
    }

    /// Dart `MemberSuggestion.referencingInterface`.
    pub fn referencing_interface(&self) -> Option<Option<ElementId>> {
        match &self.kind {
            Kind::Field {
                referencing_interface,
                ..
            }
            | Kind::Getter {
                referencing_interface,
                ..
            }
            | Kind::Method {
                referencing_interface,
                ..
            }
            | Kind::SetState {
                referencing_interface,
                ..
            }
            | Kind::Setter {
                referencing_interface,
                ..
            } => Some(*referencing_interface),
            _ => None,
        }
    }
}

/// The length of [s] in UTF-16 code units.
pub(crate) fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Dart `Element.displayName`.
pub fn display_name(ctx: &Ctx<'_>, element: ElementId) -> String {
    support::display_name(ctx, element)
}

/// Dart `FieldElement.isEnumConstant`.
pub fn is_enum_constant(ctx: &Ctx<'_>, element: ElementId) -> bool {
    use dartr_element::FragmentFlags;
    if element.tag() != dartr_element::Tag::Field {
        return false;
    }
    ctx.element_data(element)
        .and_then(|d| ctx.fragment_data(d.first_fragment))
        .is_some_and(|f| f.flags.has(FragmentFlags::FIELD_FRAGMENT_IS_ENUM_CONSTANT))
}

/// Dart `getDefaultStringParameterValue`: (text, cursor position).
pub(crate) fn default_string_parameter_value(
    ctx: &Ctx<'_>,
    ty: TypeId,
    quote: char,
) -> Option<(String, Option<usize>)> {
    match ctx.ty(ty) {
        TypeKind::Interface { element, .. } => {
            let e = element.raw();
            if ctx.is_element(e, "dart.core", "List") {
                Some(("[]".to_string(), Some(1)))
            } else if ctx.is_element(e, "dart.core", "Map") {
                Some(("{}".to_string(), Some(1)))
            } else if ctx.is_element(e, "dart.core", "String") {
                Some((format!("{quote}{quote}"), Some(1)))
            } else {
                None
            }
        }
        TypeKind::Function(f) => {
            let params: Vec<String> = ctx
                .list(f.params)
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let name = p
                        .name
                        .map(|n| ctx.name_str(n).to_string())
                        .unwrap_or_else(|| format!("p{}", i + 1));
                    format!("{}{name}", type_string(ctx, p.ty))
                })
                .collect();
            let text = format!("({}) {{  }}", params.join(", "));
            let len = utf16_len(&text);
            Some((text, Some(len - 2)))
        }
        _ => None,
    }
}

/// Dart `getTypeString`.
fn type_string(ctx: &Ctx<'_>, ty: TypeId) -> String {
    if matches!(ctx.ty(ty), TypeKind::Dynamic) {
        String::new()
    } else {
        format!("{} ", super::lsp::type_display(ctx, ty))
    }
}

/// Dart `buildClosureParameters`.
pub fn build_closure_parameters(
    ctx: &Ctx<'_>,
    function_type: TypeId,
    include_types: bool,
    include_keywords: bool,
) -> String {
    let TypeKind::Function(f) = ctx.ty(function_type) else {
        return "()".to_string();
    };
    let parameters = ctx.list(f.params);
    let mut buffer = String::from("(");
    let mut has_named = false;
    let mut has_optional_positional = false;
    let existing: Vec<String> = parameters
        .iter()
        .filter_map(|p| p.name.map(|n| ctx.name_str(n).to_string()))
        .collect();
    for (i, p) in parameters.iter().enumerate() {
        if i != 0 {
            buffer.push_str(", ");
        }
        if p.kind.is_named() && !has_named {
            has_named = true;
            buffer.push('{');
        } else if p.kind.is_optional_positional() && !has_optional_positional {
            has_optional_positional = true;
            buffer.push('[');
        }
        if include_types {
            buffer.push_str(&super::lsp::type_display(ctx, p.ty));
            buffer.push(' ');
        }
        let mut name = p
            .name
            .map(|n| ctx.name_str(n).to_string())
            .unwrap_or_default();
        if name.is_empty() {
            name = format!("p{i}");
            let mut index = 1;
            while existing.contains(&name) {
                name = format!("p{i}_{index}");
                index += 1;
            }
        }
        if include_keywords && p.kind.is_required_named() {
            buffer.push_str("required ");
        }
        buffer.push_str(&name);
    }
    if has_named {
        buffer.push('}');
    } else if has_optional_positional {
        buffer.push(']');
    }
    buffer.push(')');
    buffer
}

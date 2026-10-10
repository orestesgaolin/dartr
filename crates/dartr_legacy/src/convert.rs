use dartr_element::diagnostics::library_fragment_of;
use dartr_element::display_string::{
    DisplayOptions, default_value_code, type_display_string_with, type_parameter_display_string,
};
use dartr_element::{
    AnyElement, ConstructorElement, ConstructorFragment, Ctx, ElementFlags, ElementId,
    ExecutableElement, ExtensionElement, FId, FormalParameterElement, FragmentFlags, FragmentId,
    InterfaceElement, LibraryFragment, ParameterKind, Tag, TypeAliasElement, TypeId, TypeKind,
    VariableElement, VariableFragment,
};
use dartr_resolver::element_ext::{is_const, is_enum_constant, is_final, variable_type};
use dartr_resolver::element_metadata::{
    UnitAst, element_has, flags as meta_flags, is_deprecated_with_kind,
};
use dartr_syntax::LineInfo;

use crate::protocol::{Element, ElementKind, Location, OverriddenMember};

pub const FLAG_ABSTRACT: i64 = 0x01;
pub const FLAG_CONST: i64 = 0x02;
pub const FLAG_FINAL: i64 = 0x04;
pub const FLAG_STATIC: i64 = 0x08;
pub const FLAG_PRIVATE: i64 = 0x10;
pub const FLAG_DEPRECATED: i64 = 0x20;

pub fn make_element_flags(
    is_abstract: bool,
    is_const: bool,
    is_final: bool,
    is_static: bool,
    is_deprecated: bool,
    is_private: bool,
) -> i64 {
    let mut flags = 0;
    if is_abstract {
        flags |= FLAG_ABSTRACT;
    }
    if is_const {
        flags |= FLAG_CONST;
    }
    if is_final {
        flags |= FLAG_FINAL;
    }
    if is_static {
        flags |= FLAG_STATIC;
    }
    if is_deprecated {
        flags |= FLAG_DEPRECATED;
    }
    if is_private {
        flags |= FLAG_PRIVATE;
    }
    flags
}

pub fn is_local_element(element: ElementId) -> bool {
    matches!(
        element.tag(),
        Tag::LocalVariable
            | Tag::PatternVariable
            | Tag::BindPatternVariable
            | Tag::JoinPatternVariable
            | Tag::LocalFunction
            | Tag::Label
            | Tag::FormalParameter
            | Tag::FieldFormalParameter
            | Tag::SuperFormalParameter
    )
}

pub fn find_library_fragment(ctx: &Ctx<'_>, fragment: FragmentId) -> Option<FId<LibraryFragment>> {
    library_fragment_of(ctx, fragment)
}

/// Converts a `dartr_element::ElementId` to `protocol::ElementKind`
/// following `convertElementToElementKind` in `protocol_dart.dart`.
pub fn convert_element_kind(ctx: &Ctx<'_>, element: ElementId) -> ElementKind {
    match element.tag() {
        Tag::Class => ElementKind::CLASS,
        Tag::Constructor => ElementKind::CONSTRUCTOR,
        Tag::Dynamic | Tag::Never | Tag::MultiplyDefined => ElementKind::UNKNOWN,
        Tag::Enum => ElementKind::ENUM,
        Tag::Extension => ElementKind::EXTENSION,
        Tag::ExtensionType => ElementKind::ExtensionType,
        Tag::Field => {
            if is_enum_constant(ctx, element) {
                ElementKind::EnumConstant
            } else {
                ElementKind::FIELD
            }
        }
        Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter => {
            ElementKind::PARAMETER
        }
        Tag::TopLevelFunction | Tag::LocalFunction => ElementKind::FUNCTION,
        Tag::GenericFunctionType => ElementKind::FunctionTypeAlias,
        Tag::Getter => ElementKind::GETTER,
        Tag::Label => ElementKind::LABEL,
        Tag::Library => ElementKind::LIBRARY,
        Tag::LocalVariable
        | Tag::PatternVariable
        | Tag::BindPatternVariable
        | Tag::JoinPatternVariable => ElementKind::LocalVariable,
        Tag::Method => ElementKind::METHOD,
        Tag::Mixin => ElementKind::MIXIN,
        Tag::Prefix => ElementKind::PREFIX,
        Tag::Setter => ElementKind::SETTER,
        Tag::TopLevelVariable => ElementKind::TopLevelVariable,
        Tag::TypeAlias => ElementKind::TypeAlias,
        Tag::TypeParameter => ElementKind::TypeParameter,
    }
}

/// Converts a fragment/element to `ElementKind` following `Fragment.toPluginElementKind`
/// in `navigation_dart.dart` + `AnalyzerConverter._convertElementKind` in `analyzer_converter.dart`.
pub fn convert_navigation_target_kind(element: ElementId) -> ElementKind {
    match element.tag() {
        Tag::Class => ElementKind::CLASS,
        Tag::Constructor => ElementKind::CONSTRUCTOR,
        Tag::Field => ElementKind::FIELD,
        Tag::TopLevelFunction | Tag::LocalFunction => ElementKind::FUNCTION,
        Tag::GenericFunctionType => ElementKind::FunctionTypeAlias,
        Tag::Getter => ElementKind::GETTER,
        Tag::Label => ElementKind::LABEL,
        Tag::Library => ElementKind::LIBRARY,
        Tag::LocalVariable
        | Tag::PatternVariable
        | Tag::BindPatternVariable
        | Tag::JoinPatternVariable => ElementKind::LocalVariable,
        Tag::Method => ElementKind::METHOD,
        Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter => {
            ElementKind::PARAMETER
        }
        Tag::Prefix => ElementKind::PREFIX,
        Tag::Setter => ElementKind::SETTER,
        Tag::TopLevelVariable => ElementKind::TopLevelVariable,
        Tag::TypeAlias => ElementKind::TypeAlias,
        Tag::TypeParameter => ElementKind::TypeParameter,
        _ => ElementKind::UNKNOWN,
    }
}

/// Returns the `package:analyzer` `ElementKind.displayName` for `HoverInformation.element_kind`.
pub fn analyzer_element_kind_display_name(ctx: &Ctx<'_>, element: ElementId) -> &'static str {
    match element.tag() {
        Tag::Class => "class",
        Tag::Constructor => "constructor",
        Tag::Dynamic => "<dynamic>",
        Tag::Enum => "enum",
        Tag::Extension => "extension",
        Tag::ExtensionType => "extension type",
        Tag::Field => {
            if is_enum_constant(ctx, element) {
                "enum constant"
            } else {
                "field"
            }
        }
        Tag::TopLevelFunction | Tag::LocalFunction => "function",
        Tag::GenericFunctionType => "generic function type",
        Tag::Getter => "getter",
        Tag::Label => "label",
        Tag::Library => "library",
        Tag::LocalVariable
        | Tag::PatternVariable
        | Tag::BindPatternVariable
        | Tag::JoinPatternVariable => "local variable",
        Tag::Method => "method",
        Tag::Mixin => "mixin",
        Tag::MultiplyDefined => "<error>",
        Tag::Never => "<never>",
        Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter => "parameter",
        Tag::Prefix => "import prefix",
        Tag::Setter => "setter",
        Tag::TopLevelVariable => "top level variable",
        Tag::TypeAlias => "type alias",
        Tag::TypeParameter => "type parameter",
    }
}

/// Converts a `dartr_element::ElementId` to a `protocol::Element`,
/// following `convertElement` in `protocol_dart.dart`.
pub fn convert_element(
    ctx: &Ctx<'_>,
    element: ElementId,
    unit_ast: Option<UnitAst<'_>>,
) -> Element {
    let kind = convert_element_kind(ctx, element);
    let name = get_element_display_name(ctx, element);
    let element_type_parameters = get_type_parameters_string(ctx, element);
    let aliased_type = get_aliased_type_string(ctx, element);
    let element_parameters = get_parameters_string(ctx, element, unit_ast);
    let element_return_type = get_return_type_string(ctx, element);
    let extended_type = get_extended_type_string(ctx, element);

    let is_private = match element.tag() {
        Tag::Library | Tag::Dynamic | Tag::Never => name.starts_with('_'),
        _ => ctx
            .element_data(element)
            .and_then(|d| d.name)
            .map(|n| ctx.name_str(n).starts_with('_'))
            .unwrap_or(true),
    };
    let is_deprecated = is_deprecated_with_kind(ctx, element, "use", unit_ast);
    let is_abstract = match element.tag() {
        Tag::Class => ctx
            .element_data(element)
            .is_some_and(|d| d.flags.has(ElementFlags::CLASS_ELEMENT_IS_ABSTRACT)),
        Tag::Method => dartr_resolver::element_ext::first_fragment_flags(ctx, element)
            .contains(FragmentFlags::EXECUTABLE_FRAGMENT_IS_ABSTRACT),
        Tag::Mixin => true,
        _ => false,
    };
    let is_const_flag = match element.tag() {
        Tag::Constructor => ctx
            .element_data(element)
            .and_then(|d| d.first_fragment.cast::<ConstructorFragment>())
            .is_some_and(|f| {
                ctx.fragment(f)
                    .flags
                    .has(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_CONST)
            }),
        _ if element.cast::<VariableElement>().is_some() => is_const(ctx, element),
        _ => false,
    };
    let is_final_flag = if matches!(
        element.tag(),
        Tag::FieldFormalParameter | Tag::SuperFormalParameter
    ) {
        true
    } else if element.cast::<VariableElement>().is_some() {
        is_final(ctx, element)
    } else {
        false
    };
    let is_static_flag = match element.tag() {
        Tag::TopLevelFunction | Tag::TopLevelVariable => true,
        Tag::Method | Tag::Getter | Tag::Setter => {
            if let Some(d) = ctx.element_data(element) {
                if d.enclosing.is_some_and(|enc| enc.tag() == Tag::Library) {
                    true
                } else {
                    dartr_resolver::element_ext::first_fragment_flags(ctx, element)
                        .contains(FragmentFlags::EXECUTABLE_FRAGMENT_IS_STATIC)
                }
            } else {
                false
            }
        }
        Tag::Field => ctx
            .element_data(element)
            .and_then(|d| d.first_fragment.cast::<VariableFragment>())
            .is_some_and(|f| {
                ctx.store(f.store())
                    .variable_fragment(f)
                    .flags
                    .has(FragmentFlags::VARIABLE_FRAGMENT_IS_STATIC)
            }),
        _ => false,
    };

    Element {
        kind,
        name,
        location: new_location_from_element(ctx, element),
        flags: make_element_flags(
            is_abstract,
            is_const_flag,
            is_final_flag,
            is_static_flag,
            is_deprecated,
            is_private,
        ),
        parameters: element_parameters,
        return_type: element_return_type,
        type_parameters: element_type_parameters,
        aliased_type,
        extended_type,
    }
}

/// Converts a `LibraryFragment` to a `COMPILATION_UNIT` `protocol::Element`,
/// following `convertLibraryFragment` in `protocol_dart.dart`.
pub fn convert_library_fragment(ctx: &Ctx<'_>, fragment_id: FId<LibraryFragment>) -> Element {
    let frag = ctx.fragment(fragment_id);
    let file_path = frag.source.path.to_string();
    let short_name = std::path::Path::new(&file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&file_path)
        .to_string();
    let is_deprecated = is_deprecated_with_kind(ctx, frag.library.raw(), "use", None);
    Element {
        kind: ElementKind::CompilationUnit,
        name: short_name,
        location: Some(Location {
            file: file_path,
            offset: 0,
            length: 0,
            start_line: 1,
            start_column: 1,
            end_line: Some(1),
            end_column: Some(1),
        }),
        flags: make_element_flags(false, false, false, false, is_deprecated, false),
        parameters: None,
        return_type: None,
        type_parameters: None,
        aliased_type: None,
        extended_type: None,
    }
}

/// Follows `getElementDisplayName(Element element)` in `protocol_dart.dart`.
pub fn get_element_display_name(ctx: &Ctx<'_>, element: ElementId) -> String {
    match ctx.any(element) {
        AnyElement::Constructor(ctor) => {
            let class_name = ctor
                .enclosing
                .and_then(|enc| ctx.element_data(enc))
                .and_then(|d| d.name)
                .map(|n| ctx.name_str(n))
                .unwrap_or("");
            let ctor_name = ctor.name.map(|n| ctx.name_str(n)).unwrap_or("new");
            if ctor_name != "new" && !ctor_name.is_empty() {
                format!("{class_name}.{ctor_name}")
            } else {
                class_name.to_string()
            }
        }
        AnyElement::Library(lib) => lib
            .name
            .map(|n| ctx.name_str(n).to_string())
            .unwrap_or_default(),
        AnyElement::Class(e) => e
            .name
            .map(|n| ctx.name_str(n).to_string())
            .unwrap_or_default(),
        AnyElement::Enum(e) => e
            .name
            .map(|n| ctx.name_str(n).to_string())
            .unwrap_or_default(),
        AnyElement::Mixin(e) => e
            .name
            .map(|n| ctx.name_str(n).to_string())
            .unwrap_or_default(),
        AnyElement::Extension(e) => e
            .name
            .map(|n| ctx.name_str(n).to_string())
            .unwrap_or_default(),
        AnyElement::ExtensionType(e) => e
            .name
            .map(|n| ctx.name_str(n).to_string())
            .unwrap_or_default(),
        AnyElement::Dynamic => "dynamic".to_string(),
        AnyElement::Never => "Never".to_string(),
        _ => ctx
            .element_data(element)
            .and_then(|d| d.name)
            .map(|n| ctx.name_str(n).to_string())
            .unwrap_or_else(|| "<unnamed>".to_string()),
    }
}

fn get_type_parameters_string(ctx: &Ctx<'_>, element: ElementId) -> Option<String> {
    let type_params = if let Some(iface) = element.cast::<InterfaceElement>() {
        &ctx.interface(iface).type_params
    } else {
        let ta = element.cast::<TypeAliasElement>()?;
        &ctx.get(ta).type_params
    };
    if type_params.is_empty() {
        return None;
    }
    let joined = type_params
        .iter()
        .map(|&tp| type_parameter_display_string(ctx, tp))
        .collect::<Vec<_>>()
        .join(", ");
    Some(format!("<{joined}>"))
}

fn get_aliased_type_string(ctx: &Ctx<'_>, element: ElementId) -> Option<String> {
    let ta = element.cast::<TypeAliasElement>()?;
    let aliased_type = ctx.get(ta).aliased_type.get()?;
    Some(type_display_string_with(
        ctx,
        aliased_type,
        DisplayOptions::default(),
    ))
}

fn get_extended_type_string(ctx: &Ctx<'_>, element: ElementId) -> Option<String> {
    let ext = element.cast::<ExtensionElement>()?;
    let extended_type = ctx.get(ext).extended_type.get()?;
    Some(type_display_string_with(
        ctx,
        extended_type,
        DisplayOptions::default(),
    ))
}

fn get_return_type_string(ctx: &Ctx<'_>, element: ElementId) -> Option<String> {
    if element.tag() == Tag::Setter {
        return None;
    }
    if let Some(ctor) = element.cast::<ConstructorElement>() {
        if let Some(ret) = ctx.get(ctor).return_type.get() {
            return Some(type_display_string_with(
                ctx,
                ret,
                DisplayOptions::default(),
            ));
        }
        if let Some(enc) = ctx.get(ctor).enclosing
            && let Some(iface) = enc.cast::<InterfaceElement>()
        {
            let name = ctx
                .interface(iface)
                .name
                .map(|n| ctx.name_str(n))
                .unwrap_or("");
            let tps = &ctx.interface(iface).type_params;
            if tps.is_empty() {
                return Some(name.to_string());
            }
            let tp_names = tps
                .iter()
                .map(|&tp| ctx.get(tp).name.map(|n| ctx.name_str(n)).unwrap_or(""))
                .collect::<Vec<_>>()
                .join(", ");
            return Some(format!("{name}<{tp_names}>"));
        }
        return None;
    }
    if let Some(exec) = element.cast::<ExecutableElement>() {
        let ret = ctx
            .executable(exec)
            .return_type
            .get()
            .unwrap_or(TypeId::INVALID);
        return Some(type_display_string_with(
            ctx,
            ret,
            DisplayOptions::default(),
        ));
    }
    if element.cast::<VariableElement>().is_some() {
        let ty = variable_type(ctx, element);
        return Some(type_display_string_with(ctx, ty, DisplayOptions::default()));
    }
    if let Some(ta) = element.cast::<TypeAliasElement>() {
        let aliased_type = ctx.get(ta).aliased_type.get()?;
        if let TypeKind::Function(ft) = ctx.ty(aliased_type) {
            return Some(type_display_string_with(
                ctx,
                ft.ret,
                DisplayOptions::default(),
            ));
        }
    }
    None
}

struct ParamEntry {
    kind: ParameterKind,
    has_required_annotation: bool,
    ty: TypeId,
    name: Option<String>,
    default_code: Option<String>,
}

impl ParamEntry {
    fn rank(&self) -> i32 {
        if self.kind.is_required_named() || self.has_required_annotation {
            0
        } else if !self.kind.is_named() {
            -1
        } else {
            1
        }
    }
}

fn get_parameters_string(
    ctx: &Ctx<'_>,
    element: ElementId,
    unit_ast: Option<UnitAst<'_>>,
) -> Option<String> {
    let mut entries = Vec::new();
    if let Some(exec) = element.cast::<ExecutableElement>() {
        let exec_data = ctx.executable(exec);
        if element.tag() == Tag::Getter && exec_data.formal_params.is_empty() {
            return None;
        }
        for &param_id in &exec_data.formal_params {
            let param = ctx.get(param_id);
            let ty = param.type_.get().unwrap_or(TypeId::INVALID);
            let name = param.name.map(|n| ctx.name_str(n).to_string());
            let default_code = default_value_code(ctx, param_id);
            let has_req = element_has(ctx, param_id.raw(), meta_flags::REQUIRED, unit_ast);
            entries.push(ParamEntry {
                kind: param.kind,
                has_required_annotation: has_req,
                ty,
                name,
                default_code,
            });
        }
    } else {
        let ta = element.cast::<TypeAliasElement>()?;
        let aliased_type = ctx.get(ta).aliased_type.get()?;
        if let TypeKind::Function(ft) = *ctx.ty(aliased_type) {
            let req_count = ft.required_positional as usize;
            for (idx, param) in ctx.list(ft.params).iter().enumerate() {
                let kind = match param.kind {
                    ParameterKind::Required => {
                        if idx < req_count {
                            ParameterKind::Required
                        } else {
                            ParameterKind::Positional
                        }
                    }
                    other => other,
                };
                entries.push(ParamEntry {
                    kind,
                    has_required_annotation: false,
                    ty: param.ty,
                    name: param.name.map(|n| ctx.name_str(n).to_string()),
                    default_code: None,
                });
            }
        } else {
            return None;
        }
    }

    entries.sort_by_key(ParamEntry::rank);

    let mut sb = String::new();
    let mut close_optional = "";
    for param in &entries {
        if !sb.is_empty() {
            sb.push_str(", ");
        }
        if close_optional.is_empty() {
            if param.kind.is_named() {
                sb.push('{');
                close_optional = "}";
            } else if param.kind.is_optional_positional() {
                sb.push('[');
                close_optional = "]";
            }
        }
        if param.kind.is_required_named() {
            sb.push_str("required ");
        } else if param.has_required_annotation {
            sb.push_str("@required ");
        }
        let ty_str = type_display_string_with(ctx, param.ty, DisplayOptions::default());
        sb.push_str(&ty_str);
        if let Some(name) = &param.name
            && !name.is_empty()
        {
            sb.push(' ');
            sb.push_str(name);
        }
        if let Some(default_code) = &param.default_code {
            sb.push_str(" = ");
            sb.push_str(default_code);
        }
    }
    sb.push_str(close_optional);
    Some(format!("({sb})"))
}

/// Creates a `protocol::Location` from an element, following `newLocation_fromElement`
/// in `protocol_server.dart`.
pub fn new_location_from_element(ctx: &Ctx<'_>, element: ElementId) -> Option<Location> {
    if element.cast::<FormalParameterElement>().is_some()
        && ctx
            .element_data(element)
            .and_then(|d| d.enclosing)
            .is_none()
    {
        return None;
    }
    let first_fragment = ctx.element_data(element)?.first_fragment;
    new_location_from_fragment(ctx, first_fragment)
}

/// Creates a `protocol::Location` from a fragment, following `newLocation_fromFragment`
/// in `protocol_server.dart`.
pub fn new_location_from_fragment(ctx: &Ctx<'_>, fragment: FragmentId) -> Option<Location> {
    let lib_frag_id = library_fragment_of(ctx, fragment)?;
    let lib_frag = ctx.fragment(lib_frag_id);
    let (offset, length) = fragment_offset_and_length(ctx, fragment);
    Some(location_from_starts(
        &lib_frag.source.path,
        &lib_frag.line_starts,
        offset,
        length,
    ))
}

/// Computes `(offset, length)` of a fragment's name following `newLocation_fromFragment`
/// in `protocol_server.dart`.
pub fn fragment_offset_and_length(ctx: &Ctx<'_>, fragment: FragmentId) -> (u32, u32) {
    if let Some(ctor_frag_id) = fragment.cast::<ConstructorFragment>() {
        let ctor_frag = ctx.fragment(ctor_frag_id);
        if let Some(name_offset) = ctor_frag.name_offset {
            let len = ctor_frag
                .name
                .map(|n| ctx.name_str(n).encode_utf16().count() as u32)
                .unwrap_or(0);
            return (name_offset, len);
        }
        if let Some(type_name_offset) = ctor_frag.type_name_offset {
            let len = ctor_frag
                .type_name
                .map(|n| ctx.name_str(n).encode_utf16().count() as u32)
                .unwrap_or(0);
            return (type_name_offset, len);
        }
        return (0, 0);
    }
    if fragment.cast::<LibraryFragment>().is_some() {
        return (0, 0);
    }
    let Some(f_data) = ctx.fragment_data(fragment) else {
        return (0, 0);
    };
    let name_offset = f_data.name_offset.or(if fragment.tag() == Tag::Label {
        f_data.first_token_offset
    } else {
        None
    });
    if let Some(name_offset) = name_offset {
        let len = f_data
            .name
            .map(|n| ctx.name_str(n).encode_utf16().count() as u32)
            .unwrap_or(0);
        return (name_offset, len);
    }
    (0, 0)
}

pub fn line_col_from_starts(starts: &[u32], offset: u32) -> (u32, u32) {
    if starts.is_empty() {
        return (1, offset + 1);
    }
    let idx = starts.partition_point(|&s| s <= offset).saturating_sub(1);
    (idx as u32 + 1, offset - starts[idx] + 1)
}

pub fn location_from_starts(file: &str, starts: &[u32], offset: u32, length: u32) -> Location {
    let (start_line, start_column) = line_col_from_starts(starts, offset);
    let (end_line, end_column) = line_col_from_starts(starts, offset.saturating_add(length));
    Location {
        file: file.to_string(),
        offset: offset as i64,
        length: length as i64,
        start_line: start_line as i64,
        start_column: start_column as i64,
        end_line: Some(end_line as i64),
        end_column: Some(end_column as i64),
    }
}

pub fn location_from_line_info(
    file: &str,
    line_info: &LineInfo,
    offset: u32,
    length: u32,
) -> Location {
    let start = line_info.get_location(offset);
    let end = line_info.get_location(offset.saturating_add(length));
    Location {
        file: file.to_string(),
        offset: offset as i64,
        length: length as i64,
        start_line: start.line_number as i64,
        start_column: start.column_number as i64,
        end_line: Some(end.line_number as i64),
        end_column: Some(end.column_number as i64),
    }
}

pub fn new_location(file: String, offset: u32, length: u32, line_info: &LineInfo) -> Location {
    location_from_line_info(&file, line_info, offset, length)
}

/// Converts a member `ElementId` to `OverriddenMember` following `newOverriddenMember_fromEngine`
/// in `protocol_server.dart`.
pub fn new_overridden_member(
    ctx: &Ctx<'_>,
    member: ElementId,
    unit_ast: Option<UnitAst<'_>>,
) -> OverriddenMember {
    let element = convert_element(ctx, member, unit_ast);
    let class_name = ctx
        .element_data(member)
        .and_then(|d| d.enclosing)
        .and_then(|enc| ctx.element_data(enc))
        .and_then(|d| d.name)
        .map(|n| ctx.name_str(n).to_string())
        .unwrap_or_default();
    OverriddenMember {
        element,
        class_name,
    }
}

/// Finds the enclosing `LibraryFragment` of a part `LibraryFragment` (if any).
pub fn enclosing_library_fragment(
    ctx: &Ctx<'_>,
    lib_frag_id: FId<LibraryFragment>,
) -> Option<FId<LibraryFragment>> {
    if let Some(enc) = ctx
        .fragment(lib_frag_id)
        .enclosing_fragment
        .and_then(|f| f.cast::<LibraryFragment>())
    {
        return Some(enc);
    }
    let root = ctx.get(ctx.fragment(lib_frag_id).library).first_fragment();
    if lib_frag_id == root {
        return None;
    }
    let mut queue = std::collections::VecDeque::new();
    let mut visited = std::collections::HashSet::new();
    queue.push_back(root);
    visited.insert(root);
    while let Some(parent) = queue.pop_front() {
        for part in &ctx.fragment(parent).parts {
            if let dartr_element::DirectiveUri::Unit {
                library_fragment, ..
            } = part.directive.uri
            {
                if library_fragment == lib_frag_id {
                    return Some(parent);
                }
                if visited.insert(library_fragment) {
                    queue.push_back(library_fragment);
                }
            }
        }
    }
    Some(root)
}

/// Computes the ancestor `Element` path for `SearchResult.path`,
/// following `_computePath(Element element)` in `protocol_server.dart`.
pub fn compute_element_path(ctx: &Ctx<'_>, element: ElementId) -> Vec<Element> {
    let mut path = Vec::new();
    let Some(first_frag) = ctx.element_data(element).map(|d| d.first_fragment) else {
        return path;
    };
    let mut cur = Some(first_frag);
    while let Some(frag) = cur {
        if let Some(lib_frag_id) = frag.cast::<LibraryFragment>() {
            path.push(convert_library_fragment(ctx, lib_frag_id));
            let lib_elem = ctx.fragment(lib_frag_id).library.raw();
            path.push(convert_element(ctx, lib_elem, None));
            cur = enclosing_library_fragment(ctx, lib_frag_id).map(|f| f.raw());
            continue;
        }
        let Some(data) = ctx.fragment_data(frag) else {
            break;
        };
        if let Some(&elem) = data.element.try_get() {
            path.push(convert_element(ctx, elem, None));
        }
        cur = data.enclosing_fragment;
    }
    path
}

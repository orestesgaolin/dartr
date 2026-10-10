// Dart source: pkg/analysis_server/lib/src/lsp/handlers/handler_workspace_symbols.dart
// Dart source: pkg/analyzer/lib/src/dart/analysis/search.dart (FindDeclarations, _FindDeclarations, _FindLibraryDeclarations, _getSearchElementKind)
// Dart source: pkg/analysis_server/lib/src/lsp/mapping.dart (declarationKindToSymbolKind)

//! `workspace/symbol`: the declarations of the libraries of the search
//! files whose names match the query (Dart `FuzzyMatcher`).

use dartr_element::display_string::{DisplayOptions, element_display_string_with};
use dartr_element::{Ctx, ElementId, FragmentFlags, NoopSink, Tag};
use serde_json::{Value, json};

use super::Server;
use crate::fuzzy::FuzzyMatcher;
use crate::mapping::{self, ErrorOr};
use crate::uri::path_to_uri;

/// Dart `DeclarationKind`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DeclarationKind {
    Class,
    ClassTypeAlias,
    Constructor,
    Enum,
    EnumConstant,
    Extension,
    ExtensionType,
    Field,
    Function,
    Getter,
    Method,
    Mixin,
    Setter,
    TypeAlias,
    Variable,
}

/// Dart `declarationKindToSymbolKind`.
fn symbol_kind(supported: &[i64], kind: DeclarationKind) -> i64 {
    use DeclarationKind::*;
    let preferences: &[i64] = match kind {
        Class | ClassTypeAlias => &[5],
        Constructor => &[9],
        Enum => &[10],
        EnumConstant => &[22, 10],
        Extension | ExtensionType => &[5],
        Field => &[8],
        Function => &[12],
        Getter | Setter => &[7],
        Method => &[6],
        Mixin => &[5],
        TypeAlias => &[5],
        Variable => &[13],
    };
    preferences
        .iter()
        .copied()
        .find(|k| supported.contains(k))
        .unwrap_or(19)
}

fn first_flags(ctx: &Ctx<'_>, e: ElementId) -> FragmentFlags {
    ctx.element_data(e)
        .and_then(|d| ctx.fragment_data(d.first_fragment))
        .map(|f| f.flags.get())
        .unwrap_or_default()
}

/// Dart `_getSearchElementKind`.
fn search_element_kind(ctx: &Ctx<'_>, e: ElementId) -> Option<DeclarationKind> {
    Some(match e.tag() {
        Tag::Enum => DeclarationKind::Enum,
        Tag::ExtensionType => DeclarationKind::ExtensionType,
        Tag::Mixin => DeclarationKind::Mixin,
        Tag::Class => {
            if first_flags(ctx, e).contains(FragmentFlags::CLASS_FRAGMENT_IS_MIXIN_APPLICATION) {
                DeclarationKind::ClassTypeAlias
            } else {
                DeclarationKind::Class
            }
        }
        Tag::Constructor => DeclarationKind::Constructor,
        Tag::Extension => DeclarationKind::Extension,
        Tag::Field => {
            if dartr_resolver::element_ext::is_enum_constant(ctx, e) {
                DeclarationKind::EnumConstant
            } else {
                DeclarationKind::Field
            }
        }
        Tag::LocalFunction | Tag::TopLevelFunction => DeclarationKind::Function,
        Tag::Method => DeclarationKind::Method,
        Tag::Getter => DeclarationKind::Getter,
        Tag::Setter => DeclarationKind::Setter,
        Tag::TypeAlias => DeclarationKind::TypeAlias,
        Tag::TopLevelVariable | Tag::LocalVariable | Tag::FormalParameter => DeclarationKind::Variable,
        _ => return None,
    })
}

/// A declaration that matches (Dart `Declaration`).
struct Declaration {
    name: String,
    kind: DeclarationKind,
    file: String,
    code: (u32, u32),
    class_name: Option<String>,
    mixin_name: Option<String>,
    parameters: Option<String>,
}

/// Raised when the result has the maximum number of declarations.
struct Full;

/// Dart `_FindLibraryDeclarations`.
struct LibraryDeclarations<'m, 'o> {
    matcher: &'m mut FuzzyMatcher,
    out: &'o mut Vec<Declaration>,
    max: usize,
}

impl LibraryDeclarations<'_, '_> {
    fn name(ctx: &Ctx<'_>, e: ElementId) -> Option<String> {
        ctx.element_data(e)
            .and_then(|d| d.name)
            .map(|n| ctx.name_str(n).to_string())
    }

    fn add(&mut self, ctx: &Ctx<'_>, e: ElementId, name: String) -> Result<(), Full> {
        if self.out.len() >= self.max {
            return Err(Full);
        }
        let enclosing = ctx.element_data(e).and_then(|d| d.enclosing);
        let (mut class_name, mut mixin_name) = (None, None);
        match enclosing.map(|x| x.tag()) {
            Some(Tag::Enum) => {}
            Some(Tag::Mixin) => mixin_name = enclosing.and_then(|x| Self::name(ctx, x)),
            Some(Tag::Class) | Some(Tag::ExtensionType) => {
                class_name = enclosing.and_then(|x| Self::name(ctx, x))
            }
            _ => {}
        }
        let filtered = if e.tag() == Tag::Constructor {
            // Dart `ConstructorElement.displayName`.
            let class = enclosing.and_then(|x| Self::name(ctx, x)).unwrap_or_else(|| "<null>".into());
            if name == "new" { class } else { format!("{class}.{name}") }
        } else {
            name.clone()
        };
        if self.matcher.score(&filtered) < 0.0 {
            return Ok(());
        }
        let Some(kind) = search_element_kind(ctx, e) else {
            return Ok(());
        };
        let parameters = if crate::element_locator::is_executable(e) {
            let display = element_display_string_with(ctx, e, DisplayOptions::default());
            match display.find('(') {
                Some(i) if i > 0 => Some(display[i..].to_string()),
                _ => None,
            }
        } else {
            None
        };
        let Some(first) = ctx.element_data(e).map(|d| d.first_fragment) else {
            return Ok(());
        };
        let Some(file) = crate::navigation::fragment_path(ctx, first) else {
            return Ok(());
        };
        let Some(data) = ctx.fragment_data(first) else {
            return Ok(());
        };
        let mut location = data.name_offset;
        if location.is_none()
            && let Some(c) = first.cast::<dartr_element::ConstructorFragment>()
        {
            location = ctx.fragment(c).type_name_offset;
        }
        if location.is_none() {
            return Ok(());
        }
        self.out.push(Declaration {
            name,
            kind,
            file,
            code: (data.code_offset.unwrap_or(0), data.code_length.unwrap_or(0)),
            class_name,
            mixin_name,
            parameters,
        });
        Ok(())
    }

    fn origin(ctx: &Ctx<'_>, e: ElementId) -> bool {
        let f = first_flags(ctx, e);
        match e.tag() {
            Tag::Constructor => f.contains(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_ORIGIN_DECLARATION),
            Tag::Field | Tag::TopLevelVariable => {
                f.contains(FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_DECLARATION)
            }
            Tag::Getter | Tag::Setter => {
                f.contains(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_DECLARATION)
            }
            _ => true,
        }
    }

    fn named(&mut self, ctx: &Ctx<'_>, elements: &[ElementId], check_origin: bool, display: bool) -> Result<(), Full> {
        for &e in elements {
            if check_origin && !Self::origin(ctx, e) {
                continue;
            }
            let name = if display {
                Some(dartr_resolver::error::support::display_name(ctx, e))
            } else {
                Self::name(ctx, e)
            };
            if let Some(n) = name {
                self.add(ctx, e, n)?;
            }
        }
        Ok(())
    }

    fn members(&mut self, ctx: &Ctx<'_>, e: ElementId, with_constructors: bool) -> Result<(), Full> {
        let Some(instance) = e.cast::<dartr_element::InstanceElement>() else {
            return Ok(());
        };
        let data = ctx.instance(instance);
        let getters: Vec<ElementId> = data.getters.iter().map(|x| x.raw()).collect();
        let fields: Vec<ElementId> = data.fields.iter().map(|x| x.raw()).collect();
        let methods: Vec<ElementId> = data.methods.iter().map(|x| x.raw()).collect();
        let setters: Vec<ElementId> = data.setters.iter().map(|x| x.raw()).collect();
        if with_constructors {
            self.named(ctx, &getters, true, true)?;
            if let Some(interface) = e.cast::<dartr_element::InterfaceElement>() {
                let constructors: Vec<ElementId> =
                    ctx.interface(interface).constructors.iter().map(|x| x.raw()).collect();
                self.named(ctx, &constructors, true, false)?;
            }
            self.named(ctx, &fields, true, false)?;
            self.named(ctx, &methods, false, false)?;
            self.named(ctx, &setters, true, true)?;
        } else {
            // Dart `_addExtensions`: fields, getters, methods, setters.
            self.named(ctx, &fields, true, false)?;
            self.named(ctx, &getters, true, true)?;
            self.named(ctx, &methods, false, false)?;
            self.named(ctx, &setters, true, true)?;
        }
        Ok(())
    }

    fn classes(&mut self, ctx: &Ctx<'_>, elements: &[ElementId]) -> Result<(), Full> {
        for &e in elements {
            if let Some(name) = Self::name(ctx, e) {
                self.add(ctx, e, name)?;
                self.members(ctx, e, true)?;
            }
        }
        Ok(())
    }

    /// Dart `_FindLibraryDeclarations.compute`.
    fn compute(&mut self, ctx: &Ctx<'_>, library: dartr_element::EId<dartr_element::LibraryElement>) -> Result<(), Full> {
        if self.out.len() >= self.max {
            return Err(Full);
        }
        let l = ctx.get(library);
        let ids = |v: Vec<ElementId>| v;
        self.classes(ctx, &ids(l.classes.iter().map(|x| x.raw()).collect()))?;
        self.named(ctx, &ids(l.getters.iter().map(|x| x.raw()).collect()), true, true)?;
        self.classes(ctx, &ids(l.enums.iter().map(|x| x.raw()).collect()))?;
        self.classes(ctx, &ids(l.mixins.iter().map(|x| x.raw()).collect()))?;
        for &e in &l.extensions {
            let e = e.raw();
            if let Some(name) = Self::name(ctx, e) {
                self.add(ctx, e, name)?;
            }
            self.members(ctx, e, false)?;
        }
        self.classes(ctx, &ids(l.extension_types.iter().map(|x| x.raw()).collect()))?;
        self.named(ctx, &ids(l.setters.iter().map(|x| x.raw()).collect()), true, true)?;
        self.named(ctx, &ids(l.top_level_functions.iter().map(|x| x.raw()).collect()), false, false)?;
        self.named(ctx, &ids(l.top_level_variables.iter().map(|x| x.raw()).collect()), true, false)?;
        self.named(ctx, &ids(l.type_aliases.iter().map(|x| x.raw()).collect()), false, false)?;
        Ok(())
    }
}

impl Server {
    /// Dart `WorkspaceSymbolHandler.handle`.
    pub(crate) fn workspace_symbol(&mut self, params: &Value) -> ErrorOr<Value> {
        let query = params.get("query").and_then(Value::as_str).unwrap_or_default().to_string();
        if query.is_empty() {
            return Ok(json!([]));
        }
        let supported: Vec<i64> = match self
            .client
            .raw
            .pointer("/workspace/symbol/symbolKind/valueSet")
            .and_then(Value::as_array)
        {
            Some(list) => list.iter().filter_map(Value::as_i64).collect(),
            None => mapping::DEFAULT_SYMBOL_KINDS.collect(),
        };
        let include_dependencies = self
            .client_configuration
            .global_value()
            .get("includeDependenciesInWorkspaceSymbols")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        // Dart `FindDeclarations.compute`: the added files, then the known
        // files (after `discoverAvailableFiles`).
        self.search_scope();
        let mut entries: Vec<(String, usize)> =
            self.owned.added.iter().map(|(f, c)| (f.clone(), *c)).collect();
        if include_dependencies {
            entries.extend(self.owned.known.iter().map(|(f, c)| (f.clone(), *c)));
        }
        let Some(collection) = self.collection.take() else {
            return Ok(json!([]));
        };
        let mut matcher = FuzzyMatcher::new(&query);
        let mut declarations: Vec<Declaration> = Vec::new();
        let mut processed: Vec<(usize, String)> = Vec::new();
        for (file, owner) in entries {
            let Some(linked) = self.session.linked_library_in(&collection, owner, &file) else {
                continue;
            };
            let key = (owner, linked.library_path.clone());
            if processed.contains(&key) {
                continue;
            }
            processed.push(key);
            let sink = NoopSink;
            let features = dartr_element::FeatureSet::default();
            let ctx = linked.ctx(&sink, &features);
            let Some(library) = ctx.world.libraries.get(linked.uri.as_str()).copied() else {
                continue;
            };
            let mut finder = LibraryDeclarations {
                matcher: &mut matcher,
                out: &mut declarations,
                max: 500,
            };
            if finder.compute(&ctx, library).is_err() {
                break;
            }
        }
        self.collection = Some(collection);
        let mut out = Vec::new();
        for d in declarations {
            let Some(lines) = self.line_info_of(&d.file) else { continue };
            let container = d.class_name.clone().or(d.mixin_name.clone());
            let full_name = if d.kind == DeclarationKind::Constructor {
                let c = container.clone().unwrap_or_else(|| "null".to_string());
                if d.name == "new" { c } else { format!("{c}.{}", d.name) }
            } else {
                d.name.clone()
            };
            let params = match d.parameters.as_deref() {
                Some("") | None => "",
                Some("()") => "()",
                Some(_) => "(…)",
            };
            let mut symbol = json!({
                "name": format!("{full_name}{params}"),
                "kind": symbol_kind(&supported, d.kind),
                "location": {
                    "uri": path_to_uri(&d.file),
                    "range": mapping::to_range(&lines, d.code.0, d.code.1),
                },
            });
            if let Some(c) = container {
                symbol["containerName"] = json!(c);
            }
            out.push(symbol);
        }
        Ok(Value::Array(out))
    }
}

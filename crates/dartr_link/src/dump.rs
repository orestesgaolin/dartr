// Dart source: tools/oracle/bin/elements.dart

//! The `elements` dump: one JSON line per library that describes its element
//! model (design `docs/design/semantics.md` §5.1). A port of the oracle mode
//! `tools/oracle/bin/elements.dart`; the output is byte-identical to Dart
//! `jsonEncode` of the oracle map (same key order, escaping and numbers).
//!
//! The dump reads only the `dartr_element` model. What the model does not keep
//! (the source text of const expressions) comes from [`DumpSources`].
//!
//! The Dart getters that the oracle calls are ported as functions in the
//! section "element getters" (one function per Dart getter, named after it),
//! so that other code can use them.

use std::cmp::Ordering;
use std::fmt::Write as _;

use dartr_ast::ParameterKind;
use dartr_ast::dump::write_json_string;
use dartr_element::{
    ConstExprId, ConstantInitializer, ConstructorElement, ConstructorFragment, Ctx, DirectiveUri,
    EId, ElemRef, ElementFlags, ElementId, ExecutableElement, ExtensionElement, FId, FnParam,
    FormalParameterElement, FragmentData, FragmentFlags, FragmentId, FunctionTypeData,
    FunctionTypedElement, GetterFragment, InstanceElement, InterfaceElement, LibraryElement,
    LibraryFragment, MixinElement, Name, NamespaceCombinator, Nullability, PropertyAccessorElement,
    PropertyAccessorFragmentData, PropertyInducingElement, SetterFragment, StoreId, Tag,
    TopLevelInferenceError, TypeAliasElement, TypeId, TypeKind, TypeParameterElement,
    TypeParameterizedElement, VariableElement, VariableFragment, Variance,
};

/// Provides what the element model does not keep itself.
pub trait DumpSources {
    /// `Expression.toSource()` of a const expression (default values: Dart
    /// `defaultValueCode`).
    fn const_expr_source(&self, store: StoreId, expr: ConstExprId) -> String;
}

/// One JSON line (without newline) for library [library], `"path"` = [path].
///
/// Dart: `jsonEncode(libraryJson(path, library))`.
pub fn library_json(
    ctx: &Ctx<'_>,
    sources: &dyn DumpSources,
    path: &str,
    library: EId<LibraryElement>,
) -> String {
    let d = Dumper { ctx, sources };
    let mut out = String::new();
    d.library_json(path, library).write(&mut out);
    out
}

/// `{"path":..,"error":..}`: the line for an input that is not the defining
/// unit of a library, or that failed.
pub fn error_json(path: &str, error: &str) -> String {
    let mut o = Obj::default();
    o.put("path", Json::str(path));
    o.put("error", Json::str(error));
    let mut out = String::new();
    o.done().write(&mut out);
    out
}

/// Dart `String.compareTo`: UTF-16 code unit order (not Rust byte order,
/// which differs for characters above U+FFFF against U+E000..U+FFFF).
pub fn compare_utf16(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

// ---- JSON ----

/// A JSON value with ordered object keys (Dart `Map` literal order).
enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    fn str(s: &str) -> Json {
        Json::Str(s.to_owned())
    }

    fn opt_str<S: Into<String>>(s: Option<S>) -> Json {
        match s {
            Some(s) => Json::Str(s.into()),
            None => Json::Null,
        }
    }

    /// Dart `jsonEncode`: no spaces, keys in insertion order.
    fn write(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Int(i) => {
                let _ = write!(out, "{i}");
            }
            Json::Str(s) => write_json_string(out, s),
            Json::Arr(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write(out);
                }
                out.push(']');
            }
            Json::Obj(entries) => {
                out.push('{');
                for (i, (key, value)) in entries.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_json_string(out, key);
                    out.push(':');
                    value.write(out);
                }
                out.push('}');
            }
        }
    }
}

/// Builds a [`Json::Obj`] in key order.
#[derive(Default)]
struct Obj(Vec<(String, Json)>);

impl Obj {
    fn put(&mut self, key: &str, value: Json) {
        self.0.push((key.to_owned(), value));
    }

    fn done(self) -> Json {
        Json::Obj(self.0)
    }
}

// ---- types ----

/// Dart `typeStr`: `type.getDisplayString()`; `None` (JSON null) for a null
/// type. All type strings of the dump go through this function.
pub fn type_str(ctx: &Ctx<'_>, ty: Option<TypeId>) -> Option<String> {
    ty.map(|ty| dartr_element::type_display_string_with(ctx, ty, dartr_element::DisplayOptions::default()))
}

// ---- the dump ----

/// The fixed flag list of the oracle: `(getter, flag name)`. `"f"` holds
/// the names of the flags whose getter applies to the element (see
/// [`Dumper::flag_value`]) and is `true`, sorted by code unit order.
const FLAGS: &[(&str, &str)] = &[
    ("isAbstract", "abstract"),
    ("isBase", "base"),
    ("isFinal", "final"),
    ("isInterface", "interface"),
    ("isSealed", "sealed"),
    ("isMixinClass", "mixinClass"),
    ("isMixinApplication", "mixinApplication"),
    ("isSimplyBounded", "simplyBounded"),
    ("hasNonFinalField", "hasNonFinalField"),
    ("isConst", "const"),
    ("isFactory", "factory"),
    ("isPrimary", "primary"),
    ("isStatic", "static"),
    ("isExternal", "external"),
    ("isLate", "late"),
    ("isCovariant", "covariant"),
    ("isPromotable", "promotable"),
    ("isEnumConstant", "enumConstant"),
    ("isAsynchronous", "async"),
    ("isGenerator", "generator"),
    ("isOperator", "operator"),
    ("isExtensionTypeMember", "extensionTypeMember"),
    ("hasInitializer", "hasInitializer"),
    ("hasDefaultValue", "hasDefaultValue"),
    ("isDeclaring", "declaring"),
    ("FieldFormalParameterElement", "fieldFormal"),
    ("SuperFormalParameterElement", "superFormal"),
    ("isOriginImplicitDefault", "originImplicitDefault"),
    ("isOriginMixinApplication", "originMixinApplication"),
    ("isOriginEnumValues", "originEnumValues"),
    ("isOriginGetterSetter", "originGetterSetter"),
    ("isOriginVariable", "originVariable"),
    ("isOriginInterface", "originInterface"),
    ("isOriginLoadLibrary", "originLoadLibrary"),
    (
        "isOriginDeclaringFormalParameter",
        "originDeclaringFormalParameter",
    ),
    (
        "isOriginExtensionTypeRecovery",
        "originExtensionTypeRecovery",
    ),
    (
        "isOriginExtensionTypeRecoveryRepresentation",
        "originExtensionTypeRecoveryRepresentation",
    ),
];

struct Dumper<'c, 'a> {
    ctx: &'c Ctx<'a>,
    sources: &'c dyn DumpSources,
}

impl Dumper<'_, '_> {
    fn type_json(&self, ty: Option<TypeId>) -> Json {
        Json::opt_str(type_str(self.ctx, ty))
    }

    fn types_json(&self, types: &[TypeId]) -> Json {
        Json::Arr(types.iter().map(|t| self.type_json(Some(*t))).collect())
    }

    fn ref_json(&self, e: Option<ElementId>) -> Json {
        Json::opt_str(ref_(self.ctx, e))
    }

    /// Dart `_flagValue`: the value of the flag [getter] of [e]; `false` when
    /// [e] does not have it (the Dart type tests of the oracle).
    fn flag_value(&self, e: ElementId, getter: &str) -> bool {
        let ctx = self.ctx;
        let tag = e.tag();
        let is_class = tag == Tag::Class;
        let is_field = tag == Tag::Field;
        let is_ctor = tag == Tag::Constructor;
        let is_executable = e.is::<ExecutableElement>();
        let is_variable = e.is::<VariableElement>();
        let is_formal = e.is::<FormalParameterElement>();
        let is_property = e.is::<PropertyInducingElement>();
        let is_accessor = e.is::<PropertyAccessorElement>();
        match getter {
            "isAbstract" => (is_class || is_executable || is_field) && is_abstract(ctx, e),
            "isBase" => matches!(tag, Tag::Class | Tag::Mixin) && is_base(ctx, e),
            "isFinal" => (is_class || is_variable) && is_final(ctx, e),
            "isInterface" => is_class && is_interface(ctx, e),
            "isSealed" => is_class && is_sealed(ctx, e),
            "isMixinClass" => is_class && is_mixin_class(ctx, e),
            "isMixinApplication" => is_class && is_mixin_application(ctx, e),
            "isSimplyBounded" => e.is::<TypeParameterizedElement>() && is_simply_bounded(ctx, e),
            "hasNonFinalField" => is_class && has_non_final_field(ctx, e),
            "isConst" => (is_ctor || is_variable) && is_const(ctx, e),
            "isFactory" => is_ctor && is_factory(ctx, e),
            "isPrimary" => is_ctor && is_primary(ctx, e),
            "isStatic" => (is_executable || is_variable) && is_static(ctx, e),
            "isExternal" => {
                (is_executable || is_field || tag == Tag::TopLevelVariable) && is_external(ctx, e)
            }
            "isLate" => is_variable && is_late(ctx, e),
            "isCovariant" => (is_field || is_formal) && is_covariant(ctx, e),
            "isPromotable" => is_field && is_promotable(ctx, e),
            "isEnumConstant" => is_field && is_enum_constant(ctx, e),
            "isAsynchronous" => is_executable && is_asynchronous(ctx, e),
            "isGenerator" => is_executable && is_generator(ctx, e),
            "isOperator" => tag == Tag::Method && is_operator(ctx, e),
            "isExtensionTypeMember" => is_executable && is_extension_type_member(ctx, e),
            "hasInitializer" => is_property && has_initializer(ctx, e),
            "hasDefaultValue" => is_formal && has_default_value(ctx, self.sources, e),
            "isDeclaring" => tag == Tag::FieldFormalParameter && is_declaring(ctx, e),
            "FieldFormalParameterElement" => tag == Tag::FieldFormalParameter,
            "SuperFormalParameterElement" => tag == Tag::SuperFormalParameter,
            "isOriginImplicitDefault" => is_ctor && is_origin_implicit_default(ctx, e),
            "isOriginMixinApplication" => is_ctor && is_origin_mixin_application(ctx, e),
            "isOriginEnumValues" => is_field && is_origin_enum_values(ctx, e),
            "isOriginGetterSetter" => is_property && is_origin_getter_setter(ctx, e),
            "isOriginVariable" => is_accessor && is_origin_variable(ctx, e),
            "isOriginInterface" => {
                (tag == Tag::Method || is_accessor) && is_origin_interface(ctx, e)
            }
            "isOriginLoadLibrary" => tag == Tag::TopLevelFunction && is_origin_load_library(ctx, e),
            "isOriginDeclaringFormalParameter" => {
                is_field && is_origin_declaring_formal_parameter(ctx, e)
            }
            "isOriginExtensionTypeRecovery" => is_ctor && is_origin_extension_type_recovery(ctx, e),
            "isOriginExtensionTypeRecoveryRepresentation" => {
                is_field && is_origin_extension_type_recovery_representation(ctx, e)
            }
            _ => panic!("unknown flag getter: {getter}"),
        }
    }

    /// Dart `flagsOf`.
    fn flags_of(&self, e: ElementId) -> Json {
        let mut names: Vec<&str> = FLAGS
            .iter()
            .filter(|(getter, _)| self.flag_value(e, getter))
            .map(|(_, name)| *name)
            .collect();
        names.sort_by(|a, b| compare_utf16(a, b));
        Json::Arr(names.into_iter().map(Json::str).collect())
    }

    /// Dart `libraryJson`.
    fn library_json(&self, path: &str, library: EId<LibraryElement>) -> Json {
        let ctx = self.ctx;
        let fragments = library_fragments(ctx, library);
        let lib = ctx.get(library);
        let lang = lib.language_version.effective();

        let mut imports = Vec::new();
        let mut exports = Vec::new();
        for &fragment in &fragments {
            let unit = ctx.fragment(fragment);
            for import in &unit.library_imports {
                let prefix = import.prefix.map(|p| ctx.fragment(p));
                let mut show = Vec::new();
                let mut hide = Vec::new();
                for c in &import.combinators {
                    match c {
                        NamespaceCombinator::Show { shown_names, .. } => {
                            show.extend(shown_names.iter().map(|n| Json::str(ctx.name_str(*n))))
                        }
                        NamespaceCombinator::Hide { hidden_names, .. } => {
                            hide.extend(hidden_names.iter().map(|n| Json::str(ctx.name_str(*n))))
                        }
                    }
                }
                let mut o = Obj::default();
                o.put("uri", self.directive_uri(&import.directive.uri));
                o.put(
                    "prefix",
                    Json::opt_str(prefix.and_then(|p| p.name).map(|n| ctx.name_str(n))),
                );
                o.put("show", Json::Arr(show));
                o.put("hide", Json::Arr(hide));
                o.put(
                    "deferred",
                    Json::Bool(prefix.is_some_and(|p| p.is_deferred)),
                );
                o.put("synthetic", Json::Bool(import.is_synthetic));
                imports.push(o.done());
            }
            for export in &unit.library_exports {
                exports.push(self.directive_uri(&export.directive.uri));
            }
        }

        // Dart `exportNamespace.definedNames2`, keys sorted by `compareTo`.
        let mut export_namespace = Vec::new();
        if let Some(namespace) = lib.export_namespace.try_get() {
            let mut names: Vec<(&str, ElementId)> = namespace
                .defined_names
                .iter()
                .map(|(n, e)| (ctx.name_str(*n), *e))
                .collect();
            names.sort_by(|a, b| compare_utf16(a.0, b.0));
            for (name, e) in names {
                export_namespace.push((name.to_owned(), self.ref_json(Some(e))));
            }
        }

        // Top-level elements, ordered by (unit, offset, kind rank, list index).
        let mut top_level: Vec<ElementId> = Vec::new();
        top_level.extend(lib.classes.iter().map(|e| e.raw()));
        top_level.extend(lib.mixins.iter().map(|e| e.raw()));
        top_level.extend(lib.enums.iter().map(|e| e.raw()));
        top_level.extend(lib.extension_types.iter().map(|e| e.raw()));
        top_level.extend(lib.extensions.iter().map(|e| e.raw()));
        top_level.extend(lib.type_aliases.iter().map(|e| e.raw()));
        top_level.extend(lib.top_level_functions.iter().map(|e| e.raw()));
        top_level.extend(lib.top_level_variables.iter().map(|e| e.raw()));
        top_level.extend(lib.getters.iter().map(|e| e.raw()));
        top_level.extend(lib.setters.iter().map(|e| e.raw()));
        let unit_of = |e: ElementId| -> i64 {
            fragment_library_fragment(ctx, first_fragment(ctx, e))
                .and_then(|unit| fragments.iter().position(|f| *f == unit))
                .map_or(-1, |i| i as i64)
        };
        let sorted = sort_elements(ctx, &top_level, &unit_of);

        let units = fragments
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let mut o = Obj::default();
                o.put("path", Json::str(&ctx.fragment(*f).source.path));
                if i != 0 {
                    o.put("part", Json::Bool(true));
                }
                o.done()
            })
            .collect();

        let mut o = Obj::default();
        o.put("path", Json::str(path));
        o.put("uri", Json::str(library_uri(ctx, library)));
        o.put("lang", Json::Str(format!("{}.{}", lang.major, lang.minor)));
        o.put("units", Json::Arr(units));
        o.put("imports", Json::Arr(imports));
        o.put("exports", Json::Arr(exports));
        o.put("exportNamespace", Json::Obj(export_namespace));
        o.put(
            "elements",
            Json::Arr(
                sorted
                    .into_iter()
                    .map(|e| self.element_json(e, Some(unit_of(e))))
                    .collect(),
            ),
        );
        o.done()
    }

    /// Dart `directiveUri`: the URI of the library, else the relative URI
    /// string, else null.
    fn directive_uri(&self, uri: &DirectiveUri) -> Json {
        match uri {
            DirectiveUri::Library { library, .. } => Json::str(library_uri(self.ctx, *library)),
            DirectiveUri::None => Json::Null,
            DirectiveUri::RelativeUriString {
                relative_uri_string,
            }
            | DirectiveUri::RelativeUri {
                relative_uri_string,
                ..
            }
            | DirectiveUri::Source {
                relative_uri_string,
                ..
            }
            | DirectiveUri::Unit {
                relative_uri_string,
                ..
            } => Json::str(relative_uri_string),
        }
    }

    /// Dart `elementJson`.
    fn element_json(&self, e: ElementId, unit: Option<i64>) -> Json {
        let ctx = self.ctx;
        let (kind, _) = kind_of(e);
        let mut o = Obj::default();
        o.put("k", Json::str(kind));
        o.put("n", Json::opt_str(element_name(ctx, e)));
        if let Some(unit) = unit {
            o.put("u", Json::Int(unit));
        }
        if let Some(name_offset) = fragment_data(ctx, first_fragment(ctx, e)).name_offset {
            o.put("o", Json::Int(i64::from(name_offset)));
        }
        o.put("f", self.flags_of(e));

        if let Some(interface) = e.cast::<InterfaceElement>() {
            let data = ctx.interface(interface);
            o.put("tp", self.type_parameters_json(&data.type_params));
            o.put("super", self.type_json(data.supertype.get()));
            o.put(
                "mixins",
                self.types_json(ctx.list(data.mixins.get().unwrap_or_default())),
            );
            o.put(
                "interfaces",
                self.types_json(ctx.list(data.interfaces.get().unwrap_or_default())),
            );
            if let Some(mixin) = e.cast::<MixinElement>() {
                let m = ctx.get(mixin);
                o.put(
                    "on",
                    self.types_json(ctx.list(m.superclass_constraints.get().unwrap_or_default())),
                );
                let names = ctx
                    .fragment(m.first_fragment())
                    .super_invoked_names
                    .try_get()
                    .map(|names| names.iter().map(|n| Json::str(ctx.name_str(*n))).collect())
                    .unwrap_or_default();
                o.put("superInvoked", Json::Arr(names));
            }
            if e.tag() == Tag::ExtensionType {
                // Dart `representation` is `fields.first`, `primaryConstructor`
                // is `constructors.first`.
                let rep = data.fields.first().map(|f| variable_type(ctx, f.upcast()));
                o.put("rep", self.type_json(rep));
                o.put(
                    "primaryCtor",
                    self.ref_json(data.constructors.first().map(|c| c.raw())),
                );
            }
            o.put("members", self.members_json(e));
            return o.done();
        }
        match e.tag() {
            Tag::Extension => {
                let ext = ctx.get(EId::<ExtensionElement>::from_raw(e));
                o.put("tp", self.type_parameters_json(&ext.type_params));
                // `_extendedType` is initially `InvalidType`.
                let on = ext.extended_type.get().unwrap_or(TypeId::INVALID);
                o.put("on", self.type_json(Some(on)));
                o.put("members", self.members_json(e));
            }
            Tag::TypeAlias => {
                let alias = ctx.get(EId::<TypeAliasElement>::from_raw(e));
                o.put("tp", self.type_parameters_json(&alias.type_params));
                // Dart `aliasedType` is `_aliasedType!`; not set yet is null here.
                o.put("aliased", self.type_json(alias.aliased_type.get()));
            }
            Tag::Field | Tag::TopLevelVariable => {
                let variable = EId::<PropertyInducingElement>::from_raw(e);
                o.put("type", self.type_json(Some(variable_type(ctx, variable))));
                o.put("inf", Json::Bool(has_implicit_type(ctx, e)));
                let error = ctx
                    .property_inducing(variable)
                    .type_inference_error
                    .try_get()
                    .map(|error| match error {
                        TopLevelInferenceError::DependencyCycle { .. } => "dependencyCycle",
                        TopLevelInferenceError::OverrideNoCombinedSuperSignature { .. } => {
                            "overrideNoCombinedSuperSignature"
                        }
                    });
                o.put("typeInferenceError", Json::opt_str(error));
            }
            Tag::Getter | Tag::Setter => {
                let accessor = EId::<PropertyAccessorElement>::from_raw(e);
                let type_ = executable_type(ctx, accessor.upcast());
                o.put("type", self.type_json(Some(type_)));
                // A synthetic accessor of a variable has the type of the variable.
                let variable = ctx.property_accessor(accessor).variable.get();
                let inf = if is_origin_variable(ctx, e) {
                    variable.is_some_and(|v| has_implicit_type(ctx, v.raw()))
                } else {
                    executable_inferred(ctx, accessor.upcast())
                };
                o.put("inf", Json::Bool(inf));
                o.put("var", self.ref_json(variable.map(|v| v.raw())));
                if e.tag() == Tag::Setter {
                    o.put("params", self.parameters_json(accessor.upcast()));
                }
            }
            Tag::Constructor => {
                let ctor = EId::<ConstructorElement>::from_raw(e);
                let data = ctx.get(ctor);
                o.put(
                    "type",
                    self.type_json(Some(executable_type(ctx, ctor.upcast()))),
                );
                o.put("inf", Json::Bool(executable_inferred(ctx, ctor.upcast())));
                o.put("params", self.parameters_json(ctor.upcast()));
                let redirected = data.redirected_constructor.get();
                o.put(
                    "redirected",
                    self.ref_json(redirected.map(|r| base_element(ctx, r))),
                );
                let super_ctor = data.super_constructor.get();
                o.put(
                    "superCtor",
                    self.ref_json(super_ctor.map(|r| base_element(ctx, r))),
                );
            }
            Tag::Method | Tag::TopLevelFunction => {
                let executable = EId::<ExecutableElement>::from_raw(e);
                o.put(
                    "type",
                    self.type_json(Some(executable_type(ctx, executable))),
                );
                o.put("inf", Json::Bool(executable_inferred(ctx, executable)));
                o.put(
                    "tp",
                    self.type_parameters_json(&ctx.executable(executable).type_params),
                );
                o.put("params", self.parameters_json(executable));
            }
            _ => panic!("unexpected element: {e:?}"),
        }
        o.done()
    }

    /// Dart `membersJson`.
    fn members_json(&self, e: ElementId) -> Json {
        let ctx = self.ctx;
        let instance = ctx.instance(EId::<InstanceElement>::from_raw(e));
        let mut members: Vec<ElementId> = Vec::new();
        members.extend(instance.fields.iter().map(|m| m.raw()));
        members.extend(instance.getters.iter().map(|m| m.raw()));
        members.extend(instance.setters.iter().map(|m| m.raw()));
        if let Some(interface) = e.cast::<InterfaceElement>() {
            members.extend(
                ctx.interface(interface)
                    .constructors
                    .iter()
                    .map(|m| m.raw()),
            );
        }
        members.extend(instance.methods.iter().map(|m| m.raw()));
        let sorted = sort_elements(ctx, &members, &|_| 0);
        Json::Arr(
            sorted
                .into_iter()
                .map(|m| self.element_json(m, None))
                .collect(),
        )
    }

    /// Dart `typeParametersJson`.
    fn type_parameters_json(&self, type_params: &[EId<TypeParameterElement>]) -> Json {
        let ctx = self.ctx;
        Json::Arr(
            type_params
                .iter()
                .map(|tp| {
                    let data = ctx.get(*tp);
                    let mut o = Obj::default();
                    o.put("n", Json::opt_str(element_name(ctx, tp.raw())));
                    o.put("bound", self.type_json(data.bound.get()));
                    o.put("default", self.type_json(data.default_type.get()));
                    // `isLegacyCovariant` is `_variance == null`.
                    o.put("variance", Json::opt_str(data.variance.map(variance_name)));
                    o.done()
                })
                .collect(),
        )
    }

    /// Dart `parametersJson`.
    fn parameters_json(&self, e: EId<ExecutableElement>) -> Json {
        let ctx = self.ctx;
        // Oracle change (made together with this port): a parameter of a
        // setter with `isOriginVariable` has the `"inf"` of the setter,
        // `setter.variable.hasImplicitType`.
        let setter_variable = if e.tag() == Tag::Setter && is_origin_variable(ctx, e.raw()) {
            Some(ctx.property_accessor(EId::from_raw(e.raw())).variable.get())
        } else {
            None
        };
        Json::Arr(
            ctx.executable(e)
                .formal_params
                .iter()
                .map(|p| {
                    let raw = p.raw();
                    let kind = match ctx.get(*p).kind {
                        ParameterKind::Required => "requiredPositional",
                        ParameterKind::Positional => "optionalPositional",
                        ParameterKind::NamedRequired => "requiredNamed",
                        ParameterKind::Named => "optionalNamed",
                    };
                    let inf = match setter_variable {
                        Some(variable) => variable.is_some_and(|v| has_implicit_type(ctx, v.raw())),
                        None => has_implicit_type(ctx, raw),
                    };
                    let mut o = Obj::default();
                    o.put("n", Json::opt_str(element_name(ctx, raw)));
                    o.put("kind", Json::str(kind));
                    o.put("type", self.type_json(Some(formal_parameter_type(ctx, *p))));
                    o.put("inf", Json::Bool(inf));
                    o.put("f", self.flags_of(raw));
                    o.put(
                        "default",
                        Json::opt_str(default_value_code(ctx, self.sources, raw)),
                    );
                    o.done()
                })
                .collect(),
        )
    }
}

/// Dart `kindOf`: the kind name and the kind rank (for ties at the same
/// offset).
fn kind_of(e: ElementId) -> (&'static str, i32) {
    match e.tag() {
        Tag::Class => ("class", 0),
        Tag::Mixin => ("mixin", 1),
        Tag::Enum => ("enum", 2),
        Tag::ExtensionType => ("extensionType", 3),
        Tag::Extension => ("extension", 4),
        Tag::TypeAlias => ("typeAlias", 5),
        Tag::TopLevelFunction => ("function", 6),
        Tag::TopLevelVariable => ("topVar", 7),
        Tag::Field => ("field", 10),
        Tag::Getter => ("getter", 11),
        Tag::Setter => ("setter", 12),
        Tag::Constructor => ("ctor", 13),
        Tag::Method => ("method", 14),
        _ => panic!("unexpected element: {e:?}"),
    }
}

/// Dart `sortElements`: by (unit, firstFragment.offset, kind rank, list
/// index).
fn sort_elements(
    ctx: &Ctx<'_>,
    elements: &[ElementId],
    unit: &dyn Fn(ElementId) -> i64,
) -> Vec<ElementId> {
    let mut keyed: Vec<((i64, i64, i32, usize), ElementId)> = elements
        .iter()
        .enumerate()
        .map(|(i, &e)| {
            let offset = fragment_offset(ctx, first_fragment(ctx, e));
            ((unit(e), offset, kind_of(e).1, i), e)
        })
        .collect();
    keyed.sort_by_key(|k| k.0);
    keyed.into_iter().map(|k| k.1).collect()
}

/// Dart `executableInferred`.
fn executable_inferred(ctx: &Ctx<'_>, e: EId<ExecutableElement>) -> bool {
    has_implicit_return_type(ctx, e)
        || ctx
            .executable(e)
            .formal_params
            .iter()
            .any(|p| has_implicit_type(ctx, p.raw()))
}

/// Dart `varianceName`.
fn variance_name(v: Variance) -> &'static str {
    match v {
        Variance::Contravariant => "in",
        Variance::Covariant => "out",
        Variance::Invariant => "inout",
        Variance::Unrelated => "unrelated",
    }
}

/// Dart `ref`: R(e) = `"<libraryUri>::<path>"`, see the oracle file comment.
/// `dynamic` and `Never` have no library: `<no-library>::dynamic`.
pub fn ref_(ctx: &Ctx<'_>, e: Option<ElementId>) -> Option<String> {
    let e = e?;
    if e.tag() == Tag::MultiplyDefined {
        let name = element_name(ctx, e).unwrap_or("null");
        return Some(format!("<multiply-defined>::{name}"));
    }
    let mut names = Vec::new();
    let mut current = Some(e);
    while let Some(c) = current {
        if c.tag() == Tag::Library {
            break;
        }
        names.push(ref_name(ctx, c));
        current = ctx.element_data(c).and_then(|d| d.enclosing);
    }
    names.reverse();
    let uri = match ctx.element_data(e).and_then(|d| d.library) {
        Some(library) => library_uri(ctx, library),
        None => "<no-library>",
    };
    Some(format!("{uri}::{}", names.join(".")))
}

/// Dart `refName`.
fn ref_name(ctx: &Ctx<'_>, e: ElementId) -> String {
    let mut name = element_name(ctx, e);
    if e.tag() == Tag::Constructor && name.is_none_or(str::is_empty) {
        name = Some("new");
    }
    let name = name.unwrap_or("<unnamed>");
    if e.tag() == Tag::Setter {
        format!("{name}=")
    } else {
        name.to_owned()
    }
}

/// Dart `baseElement` of an element reference: the declaration of a
/// substituted member.
fn base_element(ctx: &Ctx<'_>, r: ElemRef) -> ElementId {
    match r {
        ElemRef::Base(e) => e,
        ElemRef::Member(m) => ctx.member(m).base,
    }
}

// ---- element getters ----
//
// Ports of the Dart getters of `pkg/analyzer/lib/src/dart/element/element.dart`
// that the dump reads. Each takes the element (or fragment) id and
// dispatches by the tag, as the Dart class hierarchy does. Call a getter only
// for elements whose Dart class has it (see `Dumper::flag_value`).

/// The data of a fragment (not `dynamic` / `Never`).
fn fragment_data<'a>(ctx: &Ctx<'a>, f: FragmentId) -> &'a FragmentData {
    ctx.fragment_data(f).expect("fragment with data")
}

/// Dart `firstFragment` (untyped).
pub fn first_fragment(ctx: &Ctx<'_>, e: ElementId) -> FragmentId {
    ctx.element_data(e)
        .expect("element with data")
        .first_fragment
}

/// Dart `fragments` of an element: the first fragment and its
/// `nextFragment` chain.
pub fn fragments(ctx: &Ctx<'_>, e: ElementId) -> Vec<FragmentId> {
    let mut result = Vec::new();
    let mut current = Some(first_fragment(ctx, e));
    while let Some(f) = current {
        result.push(f);
        current = fragment_data(ctx, f).next_fragment;
    }
    result
}

pub fn first_fragment_has(ctx: &Ctx<'_>, e: ElementId, flag: FragmentFlags) -> bool {
    fragment_data(ctx, first_fragment(ctx, e)).flags.has(flag)
}

fn all_fragments_have(ctx: &Ctx<'_>, e: ElementId, flag: FragmentFlags) -> bool {
    fragments(ctx, e)
        .into_iter()
        .all(|f| fragment_data(ctx, f).flags.has(flag))
}

fn element_has(ctx: &Ctx<'_>, e: ElementId, flag: ElementFlags) -> bool {
    ctx.element_data(e).is_some_and(|d| d.flags.has(flag))
}

/// Dart `LibraryElementImpl.fragments`: the defining unit, then the parts,
/// depth-first through `LibraryFragment.parts` with a `DirectiveUriWithUnit`.
pub fn library_fragments(ctx: &Ctx<'_>, library: EId<LibraryElement>) -> Vec<FId<LibraryFragment>> {
    fn visit_parts(
        ctx: &Ctx<'_>,
        unit: FId<LibraryFragment>,
        result: &mut Vec<FId<LibraryFragment>>,
    ) {
        for part in &ctx.fragment(unit).parts {
            if let DirectiveUri::Unit {
                library_fragment, ..
            } = &part.directive.uri
            {
                result.push(*library_fragment);
                visit_parts(ctx, *library_fragment, result);
            }
        }
    }
    let first = ctx.get(library).first_fragment();
    let mut result = vec![first];
    visit_parts(ctx, first, &mut result);
    result
}

/// Dart `LibraryElement.uri`: the URI of the source of the first fragment.
pub fn library_uri<'a>(ctx: &Ctx<'a>, library: EId<LibraryElement>) -> &'a str {
    &ctx.fragment(ctx.get(library).first_fragment()).source.uri
}

/// Dart `FragmentImpl.libraryFragment`: the enclosing library fragment.
pub fn fragment_library_fragment(ctx: &Ctx<'_>, f: FragmentId) -> Option<FId<LibraryFragment>> {
    let mut current = Some(f);
    while let Some(c) = current {
        if c.tag() == Tag::Library {
            return Some(FId::from_raw(c));
        }
        current = fragment_data(ctx, c).enclosing_fragment;
    }
    None
}

/// Dart `Fragment.offset`, for the fragment kinds that the dump sorts. A
/// missing offset where Dart uses `!` is 0 here.
pub fn fragment_offset(ctx: &Ctx<'_>, f: FragmentId) -> i64 {
    let data = fragment_data(ctx, f);
    let name_or_token = data.name_offset.or(data.first_token_offset).map(i64::from);
    let enclosing = |fallback: i64| {
        data.enclosing_fragment
            .map_or(fallback, |e| fragment_offset(ctx, e))
    };
    match f.tag() {
        // ConstructorFragmentImpl.offset
        Tag::Constructor => {
            let ctor = ctx.fragment(FId::<ConstructorFragment>::from_raw(f));
            match data
                .name_offset
                .or(ctor.type_name_offset)
                .or(data.first_token_offset)
            {
                Some(o) => i64::from(o),
                None => enclosing(0),
            }
        }
        // PropertyAccessorFragmentImpl.offset
        Tag::Getter | Tag::Setter => {
            if let Some(o) = data.name_offset {
                return i64::from(o);
            }
            if data
                .flags
                .has(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE)
                && let Some(variable) = accessor_fragment(ctx, f).inducing_variable
            {
                return fragment_offset(ctx, variable.raw());
            }
            data.first_token_offset.map_or(0, i64::from)
        }
        // FieldFragmentImpl.offset
        Tag::Field => name_or_token.unwrap_or_else(|| enclosing(0)),
        // VariableFragmentImpl.offset
        Tag::TopLevelVariable => {
            if let Some(o) = name_or_token {
                return o;
            }
            let Some(element) = data.element.try_get() else {
                return 0;
            };
            let property = ctx.property_inducing(EId::from_raw(*element));
            let accessor = property
                .getter
                .map(|g| g.raw())
                .or(property.setter.map(|s| s.raw()));
            accessor.map_or(0, |a| fragment_offset(ctx, first_fragment(ctx, a)))
        }
        // TypeParameterFragmentImpl.offset
        Tag::TypeParameter => name_or_token.unwrap_or_else(|| enclosing(-1)),
        Tag::Library | Tag::MultiplyDefined | Tag::Dynamic | Tag::Never => 0,
        // ExecutableFragmentImpl, InstanceFragmentImpl, TypeAliasFragmentImpl:
        // `nameOffset ?? firstTokenOffset!`.
        _ => name_or_token.unwrap_or(0),
    }
}

fn accessor_fragment<'a>(ctx: &Ctx<'a>, f: FragmentId) -> &'a PropertyAccessorFragmentData {
    match f.tag() {
        Tag::Getter => &ctx.fragment(FId::<GetterFragment>::from_raw(f)).accessor,
        Tag::Setter => &ctx.fragment(FId::<SetterFragment>::from_raw(f)).accessor,
        _ => unreachable!("{f:?}"),
    }
}

/// `FormalParameterElementImpl.name`: the name of the first fragment whose
/// name is not null and not `_`, else the name of the first fragment.
pub fn formal_parameter_name(ctx: &Ctx<'_>, e: ElementId) -> Option<Name> {
    let all = fragments(ctx, e);
    let named = all.iter().copied().find(|f| {
        fragment_data(ctx, *f)
            .name
            .is_some_and(|n| ctx.name_str(n) != "_")
    });
    fragment_data(ctx, named.unwrap_or(all[0])).name
}

/// Dart `Element.name`: [`formal_parameter_name`] for formal parameters,
/// `dynamic` / `Never` for the two fixed elements, else the name of the
/// element.
pub fn element_name<'a>(ctx: &Ctx<'a>, e: ElementId) -> Option<&'a str> {
    let name = match e.tag() {
        Tag::Dynamic => return Some("dynamic"),
        Tag::Never => return Some("Never"),
        Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter => {
            formal_parameter_name(ctx, e)
        }
        _ => ctx.element_data(e).and_then(|d| d.name),
    };
    name.map(|n| ctx.name_str(n))
}

/// `ClassElementImpl.isAbstract` (element flag),
/// `ExecutableElementImpl.isAbstract` (all fragments),
/// `VariableElementImpl.isAbstract` (first fragment).
pub fn is_abstract(ctx: &Ctx<'_>, e: ElementId) -> bool {
    if e.tag() == Tag::Class {
        element_has(ctx, e, ElementFlags::CLASS_ELEMENT_IS_ABSTRACT)
    } else if e.is::<ExecutableElement>() {
        all_fragments_have(ctx, e, FragmentFlags::EXECUTABLE_FRAGMENT_IS_ABSTRACT)
    } else if e.is::<VariableElement>() {
        first_fragment_has(ctx, e, FragmentFlags::VARIABLE_FRAGMENT_IS_ABSTRACT)
    } else {
        false
    }
}

/// `ClassElementImpl.isBase` (element flag), `MixinElementImpl.isBase`
/// (first fragment).
pub fn is_base(ctx: &Ctx<'_>, e: ElementId) -> bool {
    match e.tag() {
        Tag::Class => element_has(ctx, e, ElementFlags::CLASS_ELEMENT_IS_BASE),
        Tag::Mixin => first_fragment_has(ctx, e, FragmentFlags::MIXIN_FRAGMENT_IS_BASE),
        _ => false,
    }
}

/// `ClassElementImpl.isFinal` (element flag), `VariableElementImpl.isFinal`
/// (first fragment); `FieldFormalParameterElementImpl.isFinal` and
/// `SuperFormalParameterElementImpl.isFinal` are `true`.
pub fn is_final(ctx: &Ctx<'_>, e: ElementId) -> bool {
    match e.tag() {
        Tag::Class => element_has(ctx, e, ElementFlags::CLASS_ELEMENT_IS_FINAL),
        Tag::FieldFormalParameter | Tag::SuperFormalParameter => true,
        _ if e.is::<VariableElement>() => {
            first_fragment_has(ctx, e, FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL)
        }
        _ => false,
    }
}

/// `ClassElementImpl.isInterface` (element flag).
pub fn is_interface(ctx: &Ctx<'_>, e: ElementId) -> bool {
    element_has(ctx, e, ElementFlags::CLASS_ELEMENT_IS_INTERFACE)
}

/// `ClassElementImpl.isSealed` (first fragment).
pub fn is_sealed(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_has(ctx, e, FragmentFlags::CLASS_FRAGMENT_IS_SEALED)
}

/// `ClassElementImpl.isMixinClass` (first fragment).
pub fn is_mixin_class(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_has(ctx, e, FragmentFlags::CLASS_FRAGMENT_IS_MIXIN_CLASS)
}

/// `ClassElementImpl.isMixinApplication` (first fragment).
pub fn is_mixin_application(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_has(ctx, e, FragmentFlags::CLASS_FRAGMENT_IS_MIXIN_APPLICATION)
}

/// `InstanceElementImpl.isSimplyBounded`, `TypeAliasElementImpl.isSimplyBounded`
/// (element flags); `true` for executables and generic function types.
pub fn is_simply_bounded(ctx: &Ctx<'_>, e: ElementId) -> bool {
    if e.is::<InstanceElement>() {
        element_has(ctx, e, ElementFlags::INSTANCE_ELEMENT_IS_SIMPLY_BOUNDED)
    } else if e.tag() == Tag::TypeAlias {
        element_has(ctx, e, ElementFlags::TYPE_ALIAS_ELEMENT_IS_SIMPLY_BOUNDED)
    } else {
        e.is::<FunctionTypedElement>()
    }
}

/// `InterfaceElementImpl.hasNonFinalField`.
pub fn has_non_final_field(ctx: &Ctx<'_>, e: ElementId) -> bool {
    e.cast::<InterfaceElement>()
        .is_some_and(|i| ctx.interface(i).has_non_final_field.get())
}

/// `ConstructorElementImpl.isConst`, `VariableElementImpl.isConst` (first
/// fragment).
pub fn is_const(ctx: &Ctx<'_>, e: ElementId) -> bool {
    if e.tag() == Tag::Constructor {
        first_fragment_has(ctx, e, FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_CONST)
    } else {
        e.is::<VariableElement>()
            && first_fragment_has(ctx, e, FragmentFlags::VARIABLE_FRAGMENT_IS_CONST)
    }
}

/// `ConstructorElementImpl.isFactory` (first fragment).
pub fn is_factory(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_has(ctx, e, FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY)
}

/// `ConstructorElementImpl.isPrimary` (first fragment).
pub fn is_primary(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_has(ctx, e, FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_PRIMARY)
}

/// `ExecutableElementImpl.isStatic`, `VariableElementImpl.isStatic` (first
/// fragment; `TopLevelVariableFragmentImpl.isStatic` is `true`).
pub fn is_static(ctx: &Ctx<'_>, e: ElementId) -> bool {
    if e.is::<ExecutableElement>() {
        first_fragment_has(ctx, e, FragmentFlags::EXECUTABLE_FRAGMENT_IS_STATIC)
    } else if e.tag() == Tag::TopLevelVariable {
        true
    } else {
        e.is::<VariableElement>()
            && first_fragment_has(ctx, e, FragmentFlags::VARIABLE_FRAGMENT_IS_STATIC)
    }
}

/// `ExecutableElementImpl.isExternal`, `VariableElementImpl.isExternal`
/// (first fragment).
pub fn is_external(ctx: &Ctx<'_>, e: ElementId) -> bool {
    if e.is::<ExecutableElement>() {
        first_fragment_has(ctx, e, FragmentFlags::EXECUTABLE_FRAGMENT_IS_EXTERNAL)
    } else {
        e.is::<VariableElement>()
            && first_fragment_has(ctx, e, FragmentFlags::VARIABLE_FRAGMENT_IS_EXTERNAL)
    }
}

/// `VariableElementImpl.isLate` (first fragment).
pub fn is_late(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_has(ctx, e, FragmentFlags::VARIABLE_FRAGMENT_IS_LATE)
}

/// `FieldElementImpl.isCovariant` (`isExplicitlyCovariant` of the first
/// fragment), `FormalParameterElementImpl.isCovariant` (element flag).
pub fn is_covariant(ctx: &Ctx<'_>, e: ElementId) -> bool {
    match e.tag() {
        Tag::Field => first_fragment_has(
            ctx,
            e,
            FragmentFlags::FIELD_FRAGMENT_IS_EXPLICITLY_COVARIANT,
        ),
        Tag::FormalParameter | Tag::FieldFormalParameter | Tag::SuperFormalParameter => {
            element_has(ctx, e, ElementFlags::FORMAL_PARAMETER_ELEMENT_IS_COVARIANT)
        }
        _ => false,
    }
}

/// `FieldElementImpl.isPromotable` (first fragment).
pub fn is_promotable(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_has(ctx, e, FragmentFlags::FIELD_FRAGMENT_IS_PROMOTABLE)
}

/// `FieldElementImpl.isEnumConstant` (first fragment).
pub fn is_enum_constant(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_has(ctx, e, FragmentFlags::FIELD_FRAGMENT_IS_ENUM_CONSTANT)
}

/// `ExecutableFragmentImpl.isAsynchronous` of the first fragment.
pub fn is_asynchronous(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_has(ctx, e, FragmentFlags::EXECUTABLE_FRAGMENT_IS_ASYNCHRONOUS)
}

/// `ExecutableFragmentImpl.isGenerator` of the first fragment.
pub fn is_generator(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_has(ctx, e, FragmentFlags::EXECUTABLE_FRAGMENT_IS_GENERATOR)
}

/// `MethodElementImpl.isOperator` = `MethodFragmentImpl.isOperator` of the
/// first fragment: the `displayName` (`unary-` is `-`) is not empty and does
/// not start with an ASCII letter, `_` or `$`.
pub fn is_operator(ctx: &Ctx<'_>, e: ElementId) -> bool {
    let name = fragment_data(ctx, first_fragment(ctx, e))
        .name
        .map_or("", |n| ctx.name_str(n));
    let display_name = if name == "unary-" { "-" } else { name };
    match display_name.encode_utf16().next() {
        None => false,
        Some(first) => {
            !((0x61..=0x7A).contains(&first)
                || (0x41..=0x5A).contains(&first)
                || first == 0x5F
                || first == 0x24)
        }
    }
}

/// `ExecutableElementImpl.isExtensionTypeMember` (element flag).
pub fn is_extension_type_member(ctx: &Ctx<'_>, e: ElementId) -> bool {
    element_has(
        ctx,
        e,
        ElementFlags::EXECUTABLE_ELEMENT_IS_EXTENSION_TYPE_MEMBER,
    )
}

/// `PropertyInducingElementImpl.hasInitializer`: any fragment has
/// `NonParameterVariableFragmentImpl.hasInitializer`.
pub fn has_initializer(ctx: &Ctx<'_>, e: ElementId) -> bool {
    fragments(ctx, e).into_iter().any(|f| {
        fragment_data(ctx, f)
            .flags
            .has(FragmentFlags::NON_PARAMETER_VARIABLE_FRAGMENT_HAS_INITIALIZER)
    })
}

/// `VariableElementImpl.hasImplicitType` (first fragment).
pub fn has_implicit_type(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_has(ctx, e, FragmentFlags::VARIABLE_FRAGMENT_HAS_IMPLICIT_TYPE)
}

/// `ExecutableElementImpl.hasImplicitReturnType`: all fragments have it.
pub fn has_implicit_return_type(ctx: &Ctx<'_>, e: EId<ExecutableElement>) -> bool {
    all_fragments_have(
        ctx,
        e.raw(),
        FragmentFlags::EXECUTABLE_FRAGMENT_HAS_IMPLICIT_RETURN_TYPE,
    )
}

/// `FieldFormalParameterElementImpl.isDeclaring`: `isDeclaring` of the
/// first fragment that is a `FieldFormalParameterFragmentImpl`.
pub fn is_declaring(ctx: &Ctx<'_>, e: ElementId) -> bool {
    fragments(ctx, e)
        .into_iter()
        .find(|f| f.tag() == Tag::FieldFormalParameter)
        .is_some_and(|f| {
            fragment_data(ctx, f)
                .flags
                .has(FragmentFlags::FIELD_FORMAL_PARAMETER_FRAGMENT_IS_DECLARING)
        })
}

/// `ConstructorElementImpl.isOriginImplicitDefault` (first fragment).
pub fn is_origin_implicit_default(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_has(
        ctx,
        e,
        FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_ORIGIN_IMPLICIT_DEFAULT,
    )
}

/// `ConstructorElementImpl.isOriginMixinApplication` (first fragment).
pub fn is_origin_mixin_application(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_has(
        ctx,
        e,
        FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_ORIGIN_MIXIN_APPLICATION,
    )
}

/// `ConstructorElementImpl.isOriginExtensionTypeRecovery` (first fragment).
pub fn is_origin_extension_type_recovery(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_has(
        ctx,
        e,
        FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_ORIGIN_EXTENSION_TYPE_RECOVERY,
    )
}

/// `FieldElementImpl.isOriginEnumValues` (first fragment).
pub fn is_origin_enum_values(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_has(ctx, e, FragmentFlags::FIELD_FRAGMENT_IS_ORIGIN_ENUM_VALUES)
}

/// `FieldElementImpl.isOriginDeclaringFormalParameter` (first fragment).
pub fn is_origin_declaring_formal_parameter(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_has(
        ctx,
        e,
        FragmentFlags::FIELD_FRAGMENT_IS_ORIGIN_DECLARING_FORMAL_PARAMETER,
    )
}

/// `FieldElementImpl.isOriginExtensionTypeRecoveryRepresentation` (first
/// fragment).
pub fn is_origin_extension_type_recovery_representation(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_has(
        ctx,
        e,
        FragmentFlags::FIELD_FRAGMENT_IS_ORIGIN_EXTENSION_TYPE_RECOVERY_REPRESENTATION,
    )
}

/// `PropertyInducingElementImpl.isOriginGetterSetter` (first fragment).
pub fn is_origin_getter_setter(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_has(
        ctx,
        e,
        FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_GETTER_SETTER,
    )
}

/// `PropertyAccessorElementImpl.isOriginVariable` (first fragment).
pub fn is_origin_variable(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_has(
        ctx,
        e,
        FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE,
    )
}

/// `MethodElementImpl.isOriginInterface`,
/// `PropertyAccessorElementImpl.isOriginInterface` (first fragment).
pub fn is_origin_interface(ctx: &Ctx<'_>, e: ElementId) -> bool {
    match e.tag() {
        Tag::Method => {
            first_fragment_has(ctx, e, FragmentFlags::METHOD_FRAGMENT_IS_ORIGIN_INTERFACE)
        }
        Tag::Getter | Tag::Setter => first_fragment_has(
            ctx,
            e,
            FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_INTERFACE,
        ),
        _ => false,
    }
}

/// `TopLevelFunctionElementImpl.isOriginLoadLibrary` (first fragment).
pub fn is_origin_load_library(ctx: &Ctx<'_>, e: ElementId) -> bool {
    first_fragment_has(
        ctx,
        e,
        FragmentFlags::TOP_LEVEL_FUNCTION_FRAGMENT_IS_ORIGIN_LOAD_LIBRARY,
    )
}

/// `VariableElementImpl.constantInitializer2`: the cached initializer, else
/// the `constantInitializer` of the last fragment that has one.
pub fn constant_initializer2(ctx: &Ctx<'_>, e: ElementId) -> Option<ConstantInitializer> {
    let variable = ctx.variable(EId::<VariableElement>::from_raw(e));
    if let Some(Some(result)) = variable.constant_initializer.try_get() {
        return Some(*result);
    }
    fragments(ctx, e).into_iter().rev().find_map(|f| {
        let id = f.cast::<VariableFragment>()?;
        let expression = ctx
            .store(f.store())
            .variable_fragment(id)
            .constant_initializer?;
        Some(ConstantInitializer {
            fragment: f,
            expression,
        })
    })
}

/// `FormalParameterElementImpl.defaultValueCode` /
/// `SuperFormalParameterElementImpl.defaultValueCode`: the source of the
/// default value.
pub fn default_value_code(
    ctx: &Ctx<'_>,
    sources: &dyn DumpSources,
    e: ElementId,
) -> Option<String> {
    let is_super = e.tag() == Tag::SuperFormalParameter;
    if is_super
        && ctx
            .get(EId::<FormalParameterElement>::from_raw(e))
            .kind
            .is_required()
    {
        return None;
    }
    if let Some(initializer) = constant_initializer2(ctx, e) {
        let store = initializer.fragment.store();
        return Some(sources.const_expr_source(store, initializer.expression));
    }
    // Dart: `superConstructorParameter?.defaultValueCode` when
    // `_superConstructorParameterDefaultValue` is not null, that is when the
    // constant value of the super parameter's default is a subtype of the
    // type of this parameter. Interim until constant evaluation (C7): the
    // value is assumed to be a subtype unless the default is `null` and the
    // type of this parameter is non-nullable, or the super parameter has an
    // invalid type.
    if is_super {
        let p = EId::<FormalParameterElement>::from_raw(e);
        let super_parameter = crate::outline::super_constructor_parameter(ctx, p)?;
        let code = default_value_code(ctx, sources, super_parameter.raw())?;
        // A default of an unresolved type does not evaluate.
        if ctx.get(super_parameter).type_.get().is_none_or(|t| t == TypeId::INVALID) {
            return None;
        }
        if code == "null" {
            let t = ctx.get(p).type_.get().unwrap_or(TypeId::INVALID);
            if dartr_typesystem::TypeSystem::new(*ctx).is_non_nullable(t) {
                return None;
            }
        }
        return Some(code);
    }
    None
}

/// `FormalParameterElementImpl.hasDefaultValue`: `defaultValueCode != null`.
pub fn has_default_value(ctx: &Ctx<'_>, sources: &dyn DumpSources, e: ElementId) -> bool {
    default_value_code(ctx, sources, e).is_some()
}

/// `PropertyInducingElementImpl.type`; not set yet is `InvalidType` (the
/// Dart getter returns `InvalidTypeImpl.instance` when there is no type
/// inference).
pub fn variable_type(ctx: &Ctx<'_>, e: EId<PropertyInducingElement>) -> TypeId {
    ctx.property_inducing(e)
        .type_
        .get()
        .unwrap_or(TypeId::INVALID)
}

/// `FormalParameterElementImpl.type` (initially `InvalidType`).
pub fn formal_parameter_type(ctx: &Ctx<'_>, e: EId<FormalParameterElement>) -> TypeId {
    ctx.get(e).type_.get().unwrap_or(TypeId::INVALID)
}

/// `InterfaceElementImpl.thisType`: the cached type, else the element
/// instantiated with its own type parameters (not stored).
pub fn this_type(ctx: &Ctx<'_>, e: EId<InterfaceElement>) -> TypeId {
    let data = ctx.interface(e);
    if let Some(t) = data.this_type.try_get() {
        return *t;
    }
    let args: Vec<TypeId> = data
        .type_params
        .iter()
        .map(|tp| {
            ctx.intern(TypeKind::TypeParameter {
                param: *tp,
                nullability: Nullability::None,
                promoted_bound: None,
                alias: None,
            })
        })
        .collect();
    let args = ctx.intern_list(&args);
    ctx.intern(TypeKind::Interface {
        element: e,
        args,
        nullability: Nullability::None,
        alias: None,
    })
}

/// `ExecutableElementImpl.returnType`. `ConstructorElementImpl.returnType`
/// falls back to `enclosingElement.thisType`; a getter with
/// `isOriginVariable` gets the type of its variable. Else a type that is not
/// set yet is `InvalidType` (Dart: `_returnType!`).
pub fn return_type(ctx: &Ctx<'_>, e: EId<ExecutableElement>) -> TypeId {
    if let Some(t) = ctx.executable(e).return_type.get() {
        return t;
    }
    match e.tag() {
        Tag::Constructor => {
            let enclosing = ctx
                .element_data(e.raw())
                .and_then(|d| d.enclosing)
                .and_then(|x| x.cast::<InterfaceElement>());
            enclosing.map_or(TypeId::INVALID, |i| this_type(ctx, i))
        }
        Tag::Getter if is_origin_variable(ctx, e.raw()) => ctx
            .property_accessor(EId::from_raw(e.raw()))
            .variable
            .get()
            .map_or(TypeId::INVALID, |v| variable_type(ctx, v)),
        _ => TypeId::INVALID,
    }
}

/// `ExecutableElementImpl.type`: the cached type, else the function type of
/// the type parameters, formal parameters and return type, as the
/// `FunctionTypeImpl` factory builds it (positional parameters, then the
/// named ones sorted by name). The result is not stored.
pub fn executable_type(ctx: &Ctx<'_>, e: EId<ExecutableElement>) -> TypeId {
    let data = ctx.executable(e);
    if let Some(t) = data.type_.get() {
        return t;
    }
    let mut params: Vec<FnParam> = data
        .formal_params
        .iter()
        .map(|p| FnParam {
            name: formal_parameter_name(ctx, p.raw()),
            kind: ctx.get(*p).kind,
            ty: formal_parameter_type(ctx, *p),
            covariant: is_covariant(ctx, p.raw()),
            element: Some(ElemRef::Base(p.raw())),
        })
        .collect();
    if let Some(first_named) = params.iter().position(|p| p.kind.is_named()) {
        let name = |p: &FnParam| p.name.map_or("", |n| ctx.name_str(n));
        params[first_named..].sort_by(|a, b| compare_utf16(name(a), name(b)));
    }
    let required_positional = params
        .iter()
        .filter(|p| p.kind.is_required_positional())
        .count();
    let type_params = ctx.intern_list(&data.type_params);
    let params = ctx.intern_list(&params);
    ctx.intern(TypeKind::Function(FunctionTypeData {
        type_params,
        params,
        required_positional: u16::try_from(required_positional).expect("parameter count"),
        ret: return_type(ctx, e),
        nullability: Nullability::None,
        alias: None,
    }))
}

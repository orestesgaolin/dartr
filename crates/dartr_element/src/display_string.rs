// Dart source: pkg/analyzer/lib/src/dart/element/display_string_builder.dart
// (ElementDisplayStringBuilder), plus `TypeImpl.getDisplayString` and the
// `appendTo` methods of pkg/analyzer/lib/src/dart/element/type.dart, and
// `ElementImpl.displayString` and the `appendTo` / `displayName` getters of
// pkg/analyzer/lib/src/dart/element/element.dart

//! Display strings of types and elements (unit A1): the text that
//! diagnostics show for a `DartType` or an `Element` argument.
//!
//! - [`type_display_string_with`] = `TypeImpl.getDisplayString(preferTypeAlias:)`.
//! - [`element_display_string_with`] = `ElementImpl.displayString(multiline:, preferTypeAlias:)`.
//! - [`type_parameter_display_string`] = `TypeParameterElementImpl.displayString()`
//!   (variance keyword, name and bound).
//! - [`library_import_display_string`], [`library_export_display_string`],
//!   [`part_include_display_string`] = `ElementDirectiveImpl.displayString()`.
//!
//! # `_uniqueTypeParameters` without new elements
//!
//! Dart renames the type formals of a function type when their names clash
//! with the type parameters that the function type references (`T₀`, `T₁`,
//! ...). It does this with new synthetic `TypeParameterElementImpl`s and
//! `replaceTypeParameters`. This port does not create elements: the builder
//! keeps a rename map (type parameter element → display name). The new names
//! apply to the parameter types and the return type of the function type
//! (and everything in them). They do not apply to the bounds of the own
//! formals: Dart copies `typeParameter.bound` without substitution, so these
//! bounds print with the names of the enclosing scope. A synthetic Dart type
//! parameter has no variance, so the formals of a function type never show a
//! variance keyword.
//!
//! # Deviations and open points
//!
//! - Default values of formal parameters (`= code` after a parameter of an
//!   element): Dart writes `defaultValueCode`, the source text of the default
//!   value. This model only has a [`crate::ConstExprId`] for it and no source
//!   text, so the default value is not written. Open point: write it when the
//!   element model keeps the default value code.
//! - Element types that are not set yet (`VarSlot` is `None`: return types,
//!   parameter, variable and extended types) are written as `InvalidType`,
//!   the initial value of these fields in Dart.
//! - A constructor without a set return type writes the `thisType` of its
//!   enclosing element (element name and its type parameters as arguments)
//!   directly, without interning the type.
//! - `writePrefixElement` throws in Dart (`first` of an empty list) for a
//!   prefix that no import of its library fragment uses; this port writes
//!   nothing.
//! - `writeExtensionTypeElement` throws in Dart for an extension type without
//!   fields (`fields.first`); this port writes `InvalidType <null-name>` as
//!   the representation.
//! - Substituted members (`ElemRef::Member`) are not handled here (unit A7).
//! - Substitution details of `replaceTypeParameters` that change the text in
//!   rare cases (a promoted type parameter type `T & B` whose `T` is a
//!   renamed formal) are not reproduced: the rename map renames `T` and
//!   keeps `B` as written.

use crate::ctx::Ctx;
use crate::data::{DirectiveUri, LibraryExport, LibraryImport, PartInclude};
use crate::element::{ElementData, FormalParameterElement, TypeParameterElement};
use crate::flags::{ElementFlags, FragmentFlags};
use crate::ids::{EId, ElementId, FId, Tag};
use crate::name::Name;
use crate::store::AnyElement;
use crate::types::{
    AliasId, FunctionTypeData, Nullability, ParameterKind, TypeId, TypeKind, TypeList,
};
use crate::{LibraryFragment, PrefixFragment};

/// The options of `displayString` / `getDisplayString`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct DisplayOptions {
    /// Allow a display string in multiple lines (`ElementImpl.displayString(multiline:)`).
    pub multiline: bool,
    /// Write the instantiated type alias when the type has one.
    pub prefer_type_alias: bool,
}

/// `TypeImpl.getDisplayString(preferTypeAlias: ...)`.
pub fn type_display_string_with(ctx: &Ctx<'_>, ty: TypeId, options: DisplayOptions) -> String {
    let mut builder = ElementDisplayStringBuilder::new(ctx, options);
    builder.write_type(ty);
    builder.buffer
}

/// `ElementImpl.displayString(multiline:, preferTypeAlias:)`.
pub fn element_display_string_with(
    ctx: &Ctx<'_>,
    element: ElementId,
    options: DisplayOptions,
) -> String {
    let mut builder = ElementDisplayStringBuilder::new(ctx, options);
    builder.append_element(element);
    builder.buffer
}

/// `TypeParameterElementImpl.displayString()`: the variance keyword (when
/// declared), the name and the bound.
pub fn type_parameter_display_string(ctx: &Ctx<'_>, element: EId<TypeParameterElement>) -> String {
    let mut builder = ElementDisplayStringBuilder::new(ctx, DisplayOptions::default());
    builder.write_type_parameter_element(element);
    builder.buffer
}

/// `LibraryImportImpl.displayString()`: `import <uri>`.
pub fn library_import_display_string(import: &LibraryImport) -> String {
    directive_display_string("import ", &import.directive.uri)
}

/// `LibraryExportImpl.displayString()`: `export <uri>`.
pub fn library_export_display_string(export: &LibraryExport) -> String {
    directive_display_string("export ", &export.directive.uri)
}

/// `PartIncludeImpl.displayString()`: `part <uri>`.
pub fn part_include_display_string(part: &PartInclude) -> String {
    directive_display_string("part ", &part.directive.uri)
}

/// `writeLibraryImport` / `writeLibraryExport` / `writePartInclude` with
/// `_writeDirectiveUri`: the source URI for a `DirectiveUriWithSourceImpl`
/// (also `DirectiveUriWithLibraryImpl`, its subclass), else `<unknown>`.
fn directive_display_string(keyword: &str, uri: &DirectiveUri) -> String {
    let uri = match uri {
        DirectiveUri::Source { source, .. } | DirectiveUri::Library { source, .. } => &*source.uri,
        DirectiveUri::None
        | DirectiveUri::RelativeUriString { .. }
        | DirectiveUri::RelativeUri { .. }
        | DirectiveUri::Unit { .. } => "<unknown>",
    };
    format!("{keyword}{uri}")
}

/// Dart `_WriteFormalParameterKind`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum WriteFormalParameterKind {
    RequiredPositional,
    OptionalPositional,
    Named,
}

/// The data of one formal parameter that `_writeFormalParameters` reads:
/// from a `FormalParameterElement` or from a function type parameter.
#[derive(Clone)]
struct ParamView {
    kind: ParameterKind,
    ty: TypeId,
    name: Option<Name>,
    /// Dart `defaultValueCode` (elements only).
    default_code: Option<String>,
}

/// The substituted types of a member (Dart `SubstitutedElementImpl`), for
/// [`member_display_string_with`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemberTypes {
    pub return_type: Option<TypeId>,
    pub parameter_types: Vec<TypeId>,
    pub variable_type: Option<TypeId>,
}

/// `displayString(multiline:)` of a member: the element with the
/// substituted types of [types].
pub fn member_display_string_with(
    ctx: &Ctx<'_>,
    element: ElementId,
    types: &MemberTypes,
    options: DisplayOptions,
) -> String {
    let mut builder = ElementDisplayStringBuilder::new(ctx, options);
    builder.member = Some(types.clone());
    builder.append_element(element);
    builder.buffer
}

/// Dart `FormalParameterElement.defaultValueCode`: the source of the
/// default value expression (the `ConstExprs` copy).
pub fn default_value_code(ctx: &Ctx<'_>, parameter: EId<FormalParameterElement>) -> Option<String> {
    let first = ctx.element_data(parameter.raw())?.first_fragment;
    let fragment = first.cast::<crate::FormalParameterFragment>()?;
    if let Some(expression) = ctx.fragment(fragment).constant_initializer
        && let Some(const_ast) = ctx.store(first.store()).const_ast.try_get()
    {
        return Some(dartr_ast::to_source::to_source(
            const_ast.ast(),
            expression.0,
        ));
    }
    // `SuperFormalParameterElementImpl.defaultValueCode`: the default of the
    // super constructor parameter (an optional parameter only).
    if parameter.raw().tag() == crate::Tag::SuperFormalParameter
        && !ctx.get(parameter).kind.is_required()
    {
        let super_parameter = super_constructor_parameter(ctx, parameter)?;
        return default_value_code(ctx, super_parameter);
    }
    None
}

/// Dart `SuperFormalParameterElementImpl.superConstructorParameter` (base
/// element).
fn super_constructor_parameter(
    ctx: &Ctx<'_>,
    parameter: EId<FormalParameterElement>,
) -> Option<EId<FormalParameterElement>> {
    let enclosing = ctx.element_data(parameter.raw())?.enclosing?;
    let constructor = enclosing.cast::<crate::ConstructorElement>()?;
    let super_constructor = match ctx.get(constructor).super_constructor.get()? {
        crate::ElemRef::Base(b) => b,
        crate::ElemRef::Member(m) => ctx.member(m).base,
    };
    let super_params = ctx
        .executable(super_constructor.cast::<crate::ExecutableElement>()?)
        .formal_params
        .clone();
    let data = ctx.get(parameter);
    if data.kind.is_named() {
        let name = data.name;
        super_params
            .into_iter()
            .find(|&p| ctx.get(p).kind.is_named() && ctx.get(p).name == name)
    } else {
        let index = ctx
            .executable(enclosing.cast::<crate::ExecutableElement>()?)
            .formal_params
            .iter()
            .filter(|p| p.raw().tag() == crate::Tag::SuperFormalParameter)
            .position(|p| *p == parameter)?;
        super_params
            .into_iter()
            .filter(|&p| ctx.get(p).kind.is_positional())
            .nth(index)
    }
}

/// Dart `ElementDisplayStringBuilder` (with `withNullability: true`).
struct ElementDisplayStringBuilder<'c, 'a> {
    ctx: &'c Ctx<'a>,
    buffer: String,
    multiline: bool,
    prefer_type_alias: bool,
    /// The new names of the type formals of the enclosing function types
    /// (Dart: the synthetic type parameters of `_uniqueTypeParameters`).
    /// The last entry for an element wins.
    renames: Vec<(EId<TypeParameterElement>, String)>,
    /// The substituted types when the element is a member.
    member: Option<MemberTypes>,
}

impl<'c, 'a> ElementDisplayStringBuilder<'c, 'a> {
    fn new(ctx: &'c Ctx<'a>, options: DisplayOptions) -> Self {
        ElementDisplayStringBuilder {
            ctx,
            buffer: String::new(),
            multiline: options.multiline,
            prefer_type_alias: options.prefer_type_alias,
            renames: Vec::new(),
            member: None,
        }
    }

    // ---- helpers ----

    fn write(&mut self, s: &str) {
        self.buffer.push_str(s);
    }

    fn name_str(&self, name: Option<Name>) -> Option<&'a str> {
        name.map(|n| self.ctx.name_str(n))
    }

    /// `element.name ?? <fallback>`.
    fn name_or(&self, data: &ElementData, fallback: &'static str) -> &'a str {
        self.name_str(data.name).unwrap_or(fallback)
    }

    /// `ElementImpl.displayName`: `name ?? '<unnamed>'`.
    fn element_display_name(&self, data: &ElementData) -> &'a str {
        self.name_or(data, "<unnamed>")
    }

    /// `InstanceElementImpl.displayName`: `_firstFragment.displayName`, which
    /// is `name ?? ''` (the element caches the fragment name).
    fn instance_display_name(&self, data: &ElementData) -> &'a str {
        self.name_or(data, "")
    }

    /// The display name of a type parameter: the new name when an enclosing
    /// function type renamed it, else `displayName` of the element.
    fn type_parameter_display_name(&self, tp: EId<TypeParameterElement>) -> String {
        if let Some((_, name)) = self.renames.iter().rev().find(|(e, _)| *e == tp) {
            return name.clone();
        }
        self.element_display_name(self.ctx.get(tp)).to_string()
    }

    /// Dart `type ?? InvalidType` for element types that are not set yet.
    fn slot_type(ty: Option<TypeId>) -> TypeId {
        ty.unwrap_or(TypeId::INVALID)
    }

    // ---- types ----

    /// `_writeType` / `TypeImpl.appendTo`.
    fn write_type(&mut self, ty: TypeId) {
        match *self.ctx.ty(ty) {
            TypeKind::Dynamic => self.write_dynamic_type(),
            TypeKind::Void => self.write_void_type(),
            TypeKind::Invalid => self.write_invalid_type(),
            TypeKind::Unknown => self.write_unknown_inferred_type(),
            TypeKind::Never(nullability) => self.write_never_type(nullability),
            TypeKind::Interface {
                element,
                args,
                nullability,
                alias,
            } => self.write_interface_type(element.raw(), args, nullability, alias),
            TypeKind::Function(f) => self.write_function_type(f),
            TypeKind::Record {
                positional,
                named,
                nullability,
                alias,
            } => self.write_record_type(positional, named, nullability, alias),
            TypeKind::TypeParameter {
                param,
                nullability,
                promoted_bound,
                ..
            } => self.write_type_parameter_type(param, nullability, promoted_bound),
        }
    }

    fn write_dynamic_type(&mut self) {
        self.write("dynamic");
    }

    fn write_void_type(&mut self) {
        self.write("void");
    }

    fn write_invalid_type(&mut self) {
        self.write("InvalidType");
    }

    fn write_unknown_inferred_type(&mut self) {
        self.write("_");
    }

    fn write_never_type(&mut self, nullability: Nullability) {
        self.write("Never");
        self.write_nullability(nullability);
    }

    fn write_interface_type(
        &mut self,
        element: ElementId,
        args: TypeList,
        nullability: Nullability,
        alias: Option<AliasId>,
    ) {
        if self.maybe_write_type_alias(alias) {
            return;
        }
        let name = self
            .ctx
            .element_data(element)
            .and_then(|d| self.name_str(d.name))
            .unwrap_or("<null>");
        self.write(name);
        self.write_type_arguments(args);
        self.write_nullability(nullability);
    }

    fn write_record_type(
        &mut self,
        positional: TypeList,
        named: crate::types::NamedFields,
        nullability: Nullability,
        alias: Option<AliasId>,
    ) {
        if self.maybe_write_type_alias(alias) {
            return;
        }
        let positional_fields = self.ctx.list(positional);
        let named_fields = self.ctx.list(named);
        let field_count = positional_fields.len() + named_fields.len();
        self.write("(");

        let mut index = 0;
        for &field in positional_fields {
            self.write_type(field);
            if index < field_count - 1 {
                self.write(", ");
            }
            index += 1;
        }

        if !named_fields.is_empty() {
            self.write("{");
            for field in named_fields {
                self.write_type(field.ty);
                self.write(" ");
                self.write(self.ctx.name_str(field.name));
                if index < field_count - 1 {
                    self.write(", ");
                }
                index += 1;
            }
            self.write("}");
        }

        // Add trailing comma for record types with only one position field.
        if positional_fields.len() == 1 && named_fields.is_empty() {
            self.write(",");
        }

        self.write(")");
        self.write_nullability(nullability);
    }

    fn write_type_parameter_type(
        &mut self,
        param: EId<TypeParameterElement>,
        nullability: Nullability,
        promoted_bound: Option<TypeId>,
    ) {
        let name = self.type_parameter_display_name(param);
        if let Some(promoted_bound) = promoted_bound {
            let has_suffix = nullability != Nullability::None;
            if has_suffix {
                self.write("(");
            }
            self.write(&name);
            self.write(" & ");
            self.write_type(promoted_bound);
            if has_suffix {
                self.write(")");
            }
        } else {
            self.write(&name);
        }
        self.write_nullability(nullability);
    }

    fn write_function_type(&mut self, f: FunctionTypeData) {
        if self.maybe_write_type_alias(f.alias) {
            return;
        }

        // Dart: `type = _uniqueTypeParameters(type);`
        let type_params = self.ctx.list(f.type_params);
        let new_names = self.unique_type_parameters(&f);
        let base = self.renames.len();
        let push_renames = |renames: &mut Vec<(EId<TypeParameterElement>, String)>| {
            renames.extend(type_params.iter().copied().zip(new_names.iter().cloned()));
        };

        push_renames(&mut self.renames);
        self.write_type(f.ret);
        self.renames.truncate(base);

        self.write(" Function");
        // The synthetic type parameters: new name, no variance, the bound
        // copied without substitution (so without the own renames).
        if !type_params.is_empty() {
            self.write("<");
            for (i, (&tp, name)) in type_params.iter().zip(new_names.iter()).enumerate() {
                if i != 0 {
                    self.write(", ");
                }
                self.write(name);
                if let Some(bound) = self.ctx.get(tp).bound.get() {
                    self.write(" extends ");
                    self.write_type(bound);
                }
            }
            self.write(">");
        }

        push_renames(&mut self.renames);
        let params: Vec<ParamView> = self
            .ctx
            .list(f.params)
            .iter()
            .map(|p| ParamView {
                kind: p.kind,
                ty: p.ty,
                name: p.name,
                default_code: None,
            })
            .collect();
        self.write_formal_parameters(&params, false, false);
        self.renames.truncate(base);

        self.write_nullability(f.nullability);
    }

    /// Dart `_uniqueTypeParameters`: the new names of the type formals of
    /// [f], in order (empty when [f] has no type formals).
    fn unique_type_parameters(&self, f: &FunctionTypeData) -> Vec<String> {
        let own = self.ctx.list(f.type_params);
        if own.is_empty() {
            return Vec::new();
        }

        // A Dart `Set<TypeParameterElement>`; only membership matters.
        let mut referenced: Vec<EId<TypeParameterElement>> = Vec::new();
        self.collect_type_parameters_of_function(f, &mut referenced);
        referenced.retain(|e| !own.contains(e));

        let mut names_to_avoid: Vec<String> = Vec::new();
        for &tp in &referenced {
            let name = self.type_parameter_display_name(tp);
            if !names_to_avoid.contains(&name) {
                names_to_avoid.push(name);
            }
        }

        let mut new_names = Vec::with_capacity(own.len());
        for &tp in own {
            // The type parameter name can be null in erroneous cases.
            let base = self.name_str(self.ctx.get(tp).name).unwrap_or("");
            let mut name = base.to_string();
            let mut counter = 0u32;
            while names_to_avoid.contains(&name) {
                name = format!("{base}{}", subscript(counter));
                counter += 1;
            }
            names_to_avoid.push(name.clone());
            new_names.push(name);
        }
        new_names
    }

    /// `collectTypeParameters` (local function of `_uniqueTypeParameters`).
    fn collect_type_parameters(&self, ty: TypeId, out: &mut Vec<EId<TypeParameterElement>>) {
        match *self.ctx.ty(ty) {
            TypeKind::TypeParameter { param, .. } => {
                if !out.contains(&param) {
                    out.push(param);
                }
            }
            TypeKind::Function(f) => self.collect_type_parameters_of_function(&f, out),
            TypeKind::Interface { args, .. } => {
                for &arg in self.ctx.list(args) {
                    self.collect_type_parameters(arg, out);
                }
            }
            _ => {}
        }
    }

    fn collect_type_parameters_of_function(
        &self,
        f: &FunctionTypeData,
        out: &mut Vec<EId<TypeParameterElement>>,
    ) {
        for &tp in self.ctx.list(f.type_params) {
            if let Some(bound) = self.ctx.get(tp).bound.get() {
                self.collect_type_parameters(bound, out);
            }
        }
        for p in self.ctx.list(f.params) {
            self.collect_type_parameters(p.ty, out);
        }
        self.collect_type_parameters(f.ret, out);
    }

    /// `_maybeWriteTypeAlias`.
    fn maybe_write_type_alias(&mut self, alias: Option<AliasId>) -> bool {
        if self.prefer_type_alias
            && let Some(alias) = alias
        {
            let alias = *self.ctx.alias(alias);
            let name = self.name_or(self.ctx.get(alias.element), "<null>");
            self.write(name);
            self.write_type_arguments(alias.args);
            self.write_nullability(alias.nullability);
            return true;
        }
        false
    }

    /// `_writeNullability`.
    fn write_nullability(&mut self, nullability: Nullability) {
        match nullability {
            Nullability::Question => self.write("?"),
            Nullability::Star => self.write("*"),
            Nullability::None => {}
        }
    }

    /// `_writeTypeArguments`.
    fn write_type_arguments(&mut self, args: TypeList) {
        let args = self.ctx.list(args);
        if args.is_empty() {
            return;
        }
        self.write("<");
        for (i, &arg) in args.iter().enumerate() {
            if i != 0 {
                self.write(", ");
            }
            self.write_type(arg);
        }
        self.write(">");
    }

    /// `_writeTypeIfNotObject`.
    fn write_type_if_not_object(&mut self, prefix: &str, ty: Option<TypeId>) {
        if let Some(ty) = ty
            && !self.is_dart_core_object(ty)
        {
            self.write(prefix);
            self.write_type(ty);
        }
    }

    /// `TypeImpl.isDartCoreObject`: an interface type of `Object` of
    /// `dart:core` (any nullability).
    fn is_dart_core_object(&self, ty: TypeId) -> bool {
        let TypeKind::Interface { element, .. } = *self.ctx.ty(ty) else {
            return false;
        };
        let Some(data) = self.ctx.element_data(element.raw()) else {
            return false;
        };
        if self.name_str(data.name) != Some("Object") {
            return false;
        }
        // `LibraryElementImpl.isDartCore`: `name == "dart.core"`.
        data.library
            .is_some_and(|lib| self.name_str(self.ctx.get(lib).name) == Some("dart.core"))
    }

    /// `_writeTypes`.
    fn write_types(&mut self, types: TypeList) {
        for (i, &ty) in self.ctx.list(types).iter().enumerate() {
            if i != 0 {
                self.write(", ");
            }
            self.write_type(ty);
        }
    }

    /// `_writeTypesIfNotEmpty`.
    fn write_types_if_not_empty(&mut self, prefix: &str, types: Option<TypeList>) {
        let types = types.unwrap_or(TypeList::EMPTY);
        if !types.is_empty() {
            self.write(prefix);
            self.write_types(types);
        }
    }

    /// `_writeTypeParameters` (type parameters of an element).
    fn write_type_parameters(&mut self, elements: &[EId<TypeParameterElement>]) {
        if elements.is_empty() {
            return;
        }
        self.write("<");
        for (i, &tp) in elements.iter().enumerate() {
            if i != 0 {
                self.write(", ");
            }
            self.write_type_parameter_element(tp);
        }
        self.write(">");
    }

    // ---- formal parameters ----

    fn element_params(&self, params: &[EId<FormalParameterElement>]) -> Vec<ParamView> {
        let member_types = self.member.as_ref().map(|m| m.parameter_types.clone());
        params
            .iter()
            .enumerate()
            .map(|(i, &p)| {
                let data = self.ctx.get(p);
                let ty = member_types
                    .as_ref()
                    .and_then(|t| t.get(i).copied())
                    .unwrap_or_else(|| Self::slot_type(data.type_.get()));
                ParamView {
                    kind: data.kind,
                    ty,
                    name: data.name,
                    default_code: default_value_code(self.ctx, p),
                }
            })
            .collect()
    }

    /// `_writeFormalParameters`.
    fn write_formal_parameters(
        &mut self,
        parameters: &[ParamView],
        for_element: bool,
        allow_multiline: bool,
    ) {
        // Assume the display string looks better wrapped when there are at
        // least three parameters.
        let multiline = allow_multiline && self.multiline && parameters.len() >= 3;

        // The prefix for open groups is included in separator for
        // single-line but not for multiline so must be added explicitly.
        let open_group_prefix = if multiline { " " } else { "" };
        let separator = if multiline { "," } else { ", " };
        let trailing_comma = if multiline { ",\n" } else { "" };
        let parameter_prefix = if multiline { "\n  " } else { "" };

        self.write("(");

        let mut last_kind: Option<WriteFormalParameterKind> = None;
        let mut last_close = "";

        for (i, parameter) in parameters.iter().enumerate() {
            if i != 0 {
                self.write(separator);
            }

            let (kind, open, close) = if parameter.kind.is_required_positional() {
                (WriteFormalParameterKind::RequiredPositional, "", "")
            } else if parameter.kind.is_optional_positional() {
                (WriteFormalParameterKind::OptionalPositional, "[", "]")
            } else {
                (WriteFormalParameterKind::Named, "{", "}")
            };
            // Dart: openGroup(kind, open, close).
            if last_kind != Some(kind) {
                self.write(last_close);
                if last_kind.is_some() {
                    // Only include the space before the open group if there
                    // was a previous parameter.
                    self.write(open_group_prefix);
                }
                self.write(open);
                last_kind = Some(kind);
                last_close = close;
            }
            self.write(parameter_prefix);
            self.write_without_delimiters(parameter, for_element);
        }

        self.write(trailing_comma);
        self.write(last_close);
        self.write(")");
    }

    /// `_writeWithoutDelimiters`. The default value code is not written
    /// (not available in this model, see the module doc).
    fn write_without_delimiters(&mut self, parameter: &ParamView, for_element: bool) {
        if parameter.kind.is_required_named() {
            self.write("required ");
        }

        self.write_type(parameter.ty);

        if (for_element || parameter.kind.is_named())
            && let Some(name) = self.name_str(parameter.name)
        {
            self.write(" ");
            self.write(name);
        }

        if for_element && let Some(code) = &parameter.default_code {
            self.write(" = ");
            self.write(code);
        }
    }

    // ---- elements ----

    /// `ElementImpl.appendTo` and its overrides.
    fn append_element(&mut self, element: ElementId) {
        match self.ctx.any(element) {
            AnyElement::Class(_) => self.write_class_element(element),
            AnyElement::Enum(e) => {
                self.write("enum ");
                self.write(self.instance_display_name(e));
                self.write_type_parameters(&e.type_params);
                self.write_types_if_not_empty(" with ", e.mixins.get());
                self.write_types_if_not_empty(" implements ", e.interfaces.get());
            }
            AnyElement::Mixin(e) => {
                let flags = self.first_fragment_flags(e);
                if flags.contains(FragmentFlags::MIXIN_FRAGMENT_IS_BASE) {
                    self.write("base ");
                }
                self.write("mixin ");
                self.write(self.instance_display_name(e));
                self.write_type_parameters(&e.type_params);
                self.write_types_if_not_empty(" on ", e.superclass_constraints.get());
                self.write_types_if_not_empty(" implements ", e.interfaces.get());
            }
            AnyElement::Extension(e) => {
                self.write("extension");
                if let Some(name) = self.name_str(e.name) {
                    self.write(" ");
                    self.write(name);
                }
                self.write_type_parameters(&e.type_params);
                self.write(" on ");
                self.write_type(Self::slot_type(e.extended_type.get()));
            }
            AnyElement::ExtensionType(e) => {
                self.write("extension type ");
                self.write(self.instance_display_name(e));
                self.write_type_parameters(&e.type_params);
                self.write("(");
                // `representation` is `fields.first`.
                match e.fields.first() {
                    Some(&field) => {
                        let field = self.ctx.get(field);
                        self.write_type(Self::slot_type(field.type_.get()));
                        self.write(" ");
                        self.write(self.name_or(field, "<null-name>"));
                    }
                    None => self.write("InvalidType <null-name>"),
                }
                self.write(")");
                self.write_types_if_not_empty(" implements ", e.interfaces.get());
            }
            AnyElement::Field(e) => {
                self.write_variable_element(e, e.type_.get());
            }
            AnyElement::TopLevelVariable(e) => {
                self.write_variable_element(e, e.type_.get());
            }
            AnyElement::LocalVariable(e) => {
                self.write_variable_element(e, e.type_.get());
            }
            AnyElement::Getter(e) => {
                let name = format!("get {}", self.element_display_name(e));
                self.write_executable_element(e, &name, ExecutableKind::Getter);
            }
            AnyElement::Setter(e) => {
                let name = format!("set {}", self.element_display_name(e));
                self.write_executable_element(e, &name, ExecutableKind::Setter);
            }
            AnyElement::Method(e) => self.write_function_like(e, true),
            AnyElement::TopLevelFunction(e) => self.write_function_like(e, true),
            AnyElement::LocalFunction(e) => self.write_function_like(e, false),
            AnyElement::Constructor(e) => self.write_constructor_element(e),
            AnyElement::GenericFunctionType(e) => {
                self.write_type(Self::slot_type(e.return_type.get()));
                self.write(" Function");
                self.write_type_parameters(&e.type_params);
                let params = self.element_params(&e.formal_params);
                self.write_formal_parameters(&params, true, false);
            }
            AnyElement::TypeAlias(e) => {
                self.write("typedef ");
                self.write(self.element_display_name(e));
                self.write_type_parameters(&e.type_params);
                self.write(" = ");
                self.write_type(Self::slot_type(e.aliased_type.get()));
            }
            AnyElement::TypeParameter(_) => {
                self.write_type_parameter_element(EId::from_raw(element));
            }
            AnyElement::FormalParameter(e) => {
                let ty = self
                    .member
                    .as_ref()
                    .and_then(|m| m.variable_type)
                    .unwrap_or_else(|| Self::slot_type(e.type_.get()));
                let param = ParamView {
                    kind: e.kind,
                    ty,
                    name: e.name,
                    default_code: default_value_code(self.ctx, EId::from_raw(element)),
                };
                let (open, close) = if param.kind.is_required_positional() {
                    ("", "")
                } else if param.kind.is_optional_positional() {
                    ("[", "]")
                } else {
                    ("{", "}")
                };
                self.write(open);
                self.write_without_delimiters(&param, true);
                self.write(close);
            }
            AnyElement::Prefix(_) => self.write_prefix_element(EId::from_raw(element)),
            AnyElement::Library(e) => {
                // `LibraryElementImpl.uri`: `_firstFragment.source.uri`.
                let unit = self.ctx.fragment(e.first_fragment());
                self.write("library ");
                self.write(&unit.source.uri);
            }
            AnyElement::Label(e) => {
                self.write(self.name_or(e, "<null-name>"));
            }
            AnyElement::MultiplyDefined(e) => {
                // `writeAbstractElement`.
                self.write(self.name_or(e, "<unnamed MultiplyDefinedElementImpl>"));
            }
            AnyElement::Dynamic => self.write("dynamic"),
            AnyElement::Never => self.write("Never"),
        }
    }

    fn first_fragment_flags(&self, data: &ElementData) -> FragmentFlags {
        self.ctx
            .fragment_data(data.first_fragment)
            .map(|f| f.flags.get())
            .unwrap_or_default()
    }

    /// `writeClassElement`. `isAbstract`, `isBase`, `isInterface`, `isFinal`
    /// are element flags; `isSealed`, `isMixinClass` read the first fragment.
    fn write_class_element(&mut self, element: ElementId) {
        let class = self.ctx.get(EId::<crate::ClassElement>::from_raw(element));
        let flags = class.flags.get();
        let fragment_flags = self.first_fragment_flags(class);
        if fragment_flags.contains(FragmentFlags::CLASS_FRAGMENT_IS_SEALED) {
            self.write("sealed ");
        } else if flags.contains(ElementFlags::CLASS_ELEMENT_IS_ABSTRACT) {
            self.write("abstract ");
        }
        if flags.contains(ElementFlags::CLASS_ELEMENT_IS_BASE) {
            self.write("base ");
        } else if flags.contains(ElementFlags::CLASS_ELEMENT_IS_INTERFACE) {
            self.write("interface ");
        } else if flags.contains(ElementFlags::CLASS_ELEMENT_IS_FINAL) {
            self.write("final ");
        }
        if fragment_flags.contains(FragmentFlags::CLASS_FRAGMENT_IS_MIXIN_CLASS) {
            self.write("mixin ");
        }

        self.write("class ");
        self.write(self.instance_display_name(class));

        self.write_type_parameters(&class.type_params);

        self.write_type_if_not_object(" extends ", class.supertype.get());
        self.write_types_if_not_empty(" with ", class.mixins.get());
        self.write_types_if_not_empty(" implements ", class.interfaces.get());
    }

    /// `writeVariableElement`.
    fn write_variable_element(&mut self, data: &ElementData, ty: Option<TypeId>) {
        let ty = self.member.as_ref().and_then(|m| m.variable_type).or(ty);
        self.write_type(Self::slot_type(ty));
        self.write(" ");
        self.write(self.element_display_name(data));
    }

    /// `writeExecutableElement` (getters and setters).
    fn write_executable_element(
        &mut self,
        e: &crate::ExecutableElementData,
        name: &str,
        kind: ExecutableKind,
    ) {
        if kind != ExecutableKind::Setter {
            let ty = self
                .member
                .as_ref()
                .and_then(|m| m.return_type)
                .or(e.return_type.get());
            self.write_type(Self::slot_type(ty));
            self.write(" ");
        }

        self.write(name);

        if kind != ExecutableKind::Getter {
            self.write_type_parameters(&e.type_params);
            let params = self.element_params(&e.formal_params);
            self.write_formal_parameters(&params, true, true);
        }
    }

    /// `writeMethodElement`, `writeTopLevelFunctionElement` (multiline
    /// allowed) and `writeLocalFunctionElement` (not allowed).
    fn write_function_like(&mut self, e: &crate::ExecutableElementData, allow_multiline: bool) {
        let ty = self
            .member
            .as_ref()
            .and_then(|m| m.return_type)
            .or(e.return_type.get());
        self.write_type(Self::slot_type(ty));
        self.write(" ");
        self.write(self.name_or(e, "<null-name>"));
        self.write_type_parameters(&e.type_params);
        let params = self.element_params(&e.formal_params);
        self.write_formal_parameters(&params, true, allow_multiline);
    }

    /// `writeConstructorElement`.
    fn write_constructor_element(&mut self, e: &crate::ConstructorElement) {
        match self
            .member
            .as_ref()
            .and_then(|m| m.return_type)
            .or(e.return_type.get())
        {
            Some(ty) => self.write_type(ty),
            None => self.write_constructor_this_type(e.enclosing),
        }

        let display_name = self.name_or(e, "<null-name>");
        if display_name != "new" {
            self.write(".");
            self.write(display_name);
        }

        let params = self.element_params(&e.formal_params);
        self.write_formal_parameters(&params, true, true);
    }

    /// `ConstructorElementImpl.returnType` when it is not set:
    /// `enclosingElement.thisType` (the element with its type parameters as
    /// arguments, no suffix), written without interning it.
    fn write_constructor_this_type(&mut self, enclosing: Option<ElementId>) {
        let Some(enclosing) = enclosing else {
            self.write_invalid_type();
            return;
        };
        let Some(interface) = enclosing.cast::<crate::InterfaceElement>() else {
            self.write_invalid_type();
            return;
        };
        let data = self.ctx.interface(interface);
        self.write(self.name_or(data, "<null>"));
        if !data.type_params.is_empty() {
            self.write("<");
            for (i, &tp) in data.type_params.iter().enumerate() {
                if i != 0 {
                    self.write(", ");
                }
                let name = self.type_parameter_display_name(tp);
                self.write(&name);
            }
            self.write(">");
        }
    }

    /// `writeTypeParameterElement`.
    fn write_type_parameter_element(&mut self, tp: EId<TypeParameterElement>) {
        let data = self.ctx.get(tp);
        // `!isLegacyCovariant && variance != Variance.unrelated`.
        if let Some(variance) = data.variance
            && variance != crate::types::Variance::Unrelated
        {
            self.write(variance.keyword());
            self.write(" ");
        }

        self.write(self.element_display_name(data));

        if let Some(bound) = data.bound.get() {
            self.write(" extends ");
            self.write_type(bound);
        }
    }

    /// `writePrefixElement`: one `import '<uri>' as <prefix>;` line per
    /// import of the first fragment's library fragment that uses this
    /// prefix (`PrefixElementImpl.imports`).
    fn write_prefix_element(&mut self, prefix: EId<crate::PrefixElement>) {
        let data = self.ctx.get(prefix);
        let display_name = self.element_display_name(data);
        let fragment = self.ctx.fragment(data.first_fragment());
        let Some(unit) = fragment
            .enclosing_fragment
            .filter(|f| f.tag() == Tag::Library)
        else {
            return;
        };
        let unit = self.ctx.fragment(FId::<LibraryFragment>::from_raw(unit));
        let mut first = true;
        for import in &unit.library_imports {
            let uses_prefix = import.prefix.is_some_and(|p: FId<PrefixFragment>| {
                self.ctx.fragment(p).element.try_get() == Some(&prefix.raw())
            });
            if !uses_prefix {
                continue;
            }
            // `LibraryImportImpl.libraryName`.
            let library_name: &str = match &import.directive.uri {
                DirectiveUri::None => "<unknown>",
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
                | DirectiveUri::Library {
                    relative_uri_string,
                    ..
                }
                | DirectiveUri::Unit {
                    relative_uri_string,
                    ..
                } => relative_uri_string,
            };
            if !first {
                self.write("\n");
            }
            first = false;
            self.write("import '");
            self.write(library_name);
            self.write("' as ");
            self.write(display_name);
            self.write(";");
        }
    }
}

/// `element.kind` as `writeExecutableElement` tests it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ExecutableKind {
    Getter,
    Setter,
}

/// The Unicode subscript digits of [counter] (`₀` = U+2080).
fn subscript(counter: u32) -> String {
    counter
        .to_string()
        .chars()
        .map(|c| char::from_u32(0x2080 + (c as u32 - '0' as u32)).unwrap())
        .collect()
}

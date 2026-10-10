// Dart source: pkg/analyzer_plugin/lib/src/utilities/change_builder/change_builder_dart.dart (DartEditBuilderImpl: writeType, canWriteType, _writeType, _getVisibleType, _writeLibraryReference, writeParameterMatchingArgument, writeParametersMatchingArguments, _getParameterNameSuggestions; DartFileEditBuilderImpl._getImportElement, _defaultImportPrefixFor)
// Dart source: pkg/analysis_server/lib/src/services/correction/name_suggestion.dart (getCamelWords)

//! The Dart part of the edit builder: writing types (with the imports of
//! the elements they reference) and parameters that match arguments.

use dartr_ast::*;
use dartr_element::{
    Ctx, EId, ElementId, InterfaceElement, LibraryElement, NoopSink, Nullability, Tag, TypeId,
    TypeKind, TypeParameterElement,
};
use dartr_typesystem::type_ext::TypeExt;

use super::change::LinkedEditSuggestionKind;
use super::change_builder::EditBuilder;
use super::imports::{existing_imports, library_uri};

/// The options of `writeType`.
#[derive(Clone, Default)]
pub struct WriteType {
    pub add_supertype_proposals: bool,
    pub group_name: Option<String>,
    pub type_params_in_scope: Option<Vec<EId<TypeParameterElement>>>,
    pub required: bool,
    pub should_write_dynamic: bool,
    pub no_type_arguments: bool,
}

/// The enclosing class and executable at an offset (Dart
/// `_EnclosingElementFinder`).
fn enclosing_elements(
    ctx: &Ctx<'_>,
    b: &EditBuilder<'_, '_, '_>,
) -> (Option<ElementId>, Option<ElementId>) {
    let resolved = b.file.resolved();
    let unit = resolved.unit();
    let ast = &unit.ast;
    let locator = crate::element_locator::Unit {
        ctx,
        ast,
        tables: &unit.tables,
    };
    let mut class = None;
    let mut executable = None;
    let mut node = ast.node_covering(unit.unit, b.offset, 0);
    while let Some(n) = node {
        if executable.is_none()
            && (ast.is::<MethodDeclaration>(n)
                || ast.is::<FunctionDeclaration>(n)
                || ast.is::<ConstructorDeclaration>(n))
        {
            executable = locator.declared_element(n);
        }
        if class.is_none()
            && (ast.is::<ClassDeclaration>(n)
                || ast.is::<MixinDeclaration>(n)
                || ast.is::<EnumDeclaration>(n)
                || ast.is::<ExtensionDeclaration>(n)
                || ast.is::<ExtensionTypeDeclaration>(n))
        {
            class = locator.declared_element(n);
        }
        node = ast.parent(n);
    }
    (class, executable)
}

fn with_nullability(ctx: &Ctx<'_>, ty: TypeId, nullability: Nullability) -> TypeId {
    dartr_typesystem::type_ext::TypeExt::with_nullability(ctx, ty, nullability)
}

fn nullability_of(ctx: &Ctx<'_>, ty: TypeId) -> Nullability {
    match *ctx.ty(ty) {
        TypeKind::Never(n) => n,
        TypeKind::Interface { nullability, .. } => nullability,
        TypeKind::Function(f) => f.nullability,
        TypeKind::Record { nullability, .. } => nullability,
        TypeKind::TypeParameter { nullability, .. } => nullability,
        _ => Nullability::None,
    }
}

impl EditBuilder<'_, '_, '_> {
    /// The library of the unit of the builder.
    fn library(&self) -> EId<LibraryElement> {
        self.file.resolved().library.library.library
    }

    /// Dart `_isDefinedLocally`.
    fn is_defined_locally(&self, ctx: &Ctx<'_>, element: ElementId) -> bool {
        dartr_resolver::error::support::library_of(ctx, element) == Some(self.library())
    }

    /// Dart `_getVisibleType`.
    fn visible_type(
        &self,
        ctx: &Ctx<'_>,
        ty: TypeId,
        in_scope: &[EId<TypeParameterElement>],
    ) -> TypeId {
        match *ctx.ty(ty) {
            TypeKind::Interface {
                element,
                nullability,
                ..
            } => {
                let name = ctx.element_name(element.raw()).unwrap_or("");
                if name.starts_with('_') && !self.is_defined_locally(ctx, element.raw()) {
                    if let Some(supertype) = ctx.interface(element).supertype.get() {
                        let t = with_nullability(ctx, supertype, nullability);
                        return self.visible_type(ctx, t, in_scope);
                    }
                    let object = ctx.tp.object_type();
                    return with_nullability(ctx, object, nullability);
                }
                ty
            }
            TypeKind::TypeParameter {
                param, nullability, ..
            } => {
                if in_scope.contains(&param) {
                    return ty;
                }
                let mut enclosing = ctx.element_data(param.raw()).and_then(|d| d.enclosing);
                while let Some(e) = enclosing {
                    if !matches!(e.tag(), Tag::GenericFunctionType | Tag::FormalParameter) {
                        break;
                    }
                    enclosing = ctx.element_data(e).and_then(|d| d.enclosing);
                }
                let (class, executable) = enclosing_elements(ctx, self);
                if enclosing.is_some() && (enclosing == executable || enclosing == class) {
                    return ty;
                }
                let bound = ctx.get(param).bound.get();
                let t = match bound {
                    Some(b) => with_nullability(ctx, b, nullability),
                    None => ctx.tp.object_question_type(),
                };
                self.visible_type(ctx, t, in_scope)
            }
            _ => ty,
        }
    }

    /// Dart `_canWriteType`.
    fn can_write_type_impl(
        &self,
        ctx: &Ctx<'_>,
        ty: TypeId,
        in_scope: &[EId<TypeParameterElement>],
    ) -> bool {
        let ty = self.visible_type(ctx, ty, in_scope);
        match *ctx.ty(ty) {
            TypeKind::Invalid | TypeKind::Unknown => false,
            TypeKind::Dynamic
            | TypeKind::Never(_)
            | TypeKind::TypeParameter { .. }
            | TypeKind::Void => true,
            TypeKind::Interface { alias, args, .. }
            | TypeKind::Record {
                alias,
                positional: args,
                ..
            } if alias.is_some_and(|a| {
                let element = ctx.alias(a).element;
                dartr_typesystem::member::is_accessible_in(
                    ctx,
                    dartr_element::ElemRef::Base(element.raw()),
                    self.library(),
                )
            }) =>
            {
                let _ = args;
                true
            }
            TypeKind::Interface { args, .. } => {
                let mut scope: Vec<EId<TypeParameterElement>> = in_scope.to_vec();
                for &a in ctx.list(args) {
                    if let TypeKind::TypeParameter { param, .. } = *ctx.ty(a) {
                        scope.push(param);
                    }
                }
                ctx.list(args)
                    .iter()
                    .all(|&a| self.can_write_type_impl(ctx, a, &scope))
            }
            TypeKind::Function(f) => {
                let mut scope: Vec<EId<TypeParameterElement>> = ctx.list(f.type_params).to_vec();
                scope.extend_from_slice(in_scope);
                self.can_write_type_impl(ctx, f.ret, &scope)
                    && ctx.list(f.type_params).iter().all(|tp| {
                        scope.contains(tp)
                            || ctx
                                .get(*tp)
                                .bound
                                .get()
                                .is_none_or(|b| self.can_write_type_impl(ctx, b, &scope))
                    })
                    && ctx
                        .list(f.params)
                        .iter()
                        .all(|p| self.can_write_type_impl(ctx, p.ty, &scope))
            }
            TypeKind::Record {
                positional, named, ..
            } => {
                ctx.features.is_enabled("records")
                    && ctx
                        .list(positional)
                        .iter()
                        .all(|&t| self.can_write_type_impl(ctx, t, in_scope))
                    && ctx
                        .list(named)
                        .iter()
                        .all(|n| self.can_write_type_impl(ctx, n.ty, in_scope))
            }
        }
    }

    /// Dart `canWriteType`.
    pub fn can_write_type(
        &mut self,
        ty: Option<TypeId>,
        in_scope: &[EId<TypeParameterElement>],
    ) -> bool {
        let Some(ty) = ty else { return false };
        let resolved = self.file.resolved();
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        self.can_write_type_impl(&ctx, ty, in_scope)
    }

    /// Dart `writeType`: whether a type was written.
    pub fn write_type(&mut self, ty: Option<TypeId>, options: &WriteType) -> bool {
        let resolved = self.file.resolved();
        let sink = NoopSink;
        let ctx = resolved.ctx(&sink);
        let in_scope = options.type_params_in_scope.clone().unwrap_or_default();
        let mut wrote = false;
        if let Some(ty) = ty {
            if let Some(group) = &options.group_name {
                self.add_linked_edit(group, |b| {
                    wrote = b.write_type_if_can(
                        &ctx,
                        ty,
                        &in_scope,
                        options.should_write_dynamic,
                        !options.no_type_arguments,
                        None,
                    );
                    if wrote && options.add_supertype_proposals {
                        let mut added = Vec::new();
                        b.add_super_type_proposals(&ctx, Some(ty), &mut added);
                    }
                });
            } else {
                wrote = self.write_type_if_can(
                    &ctx,
                    ty,
                    &in_scope,
                    options.should_write_dynamic,
                    !options.no_type_arguments,
                    None,
                );
            }
        }
        if !wrote && options.required {
            self.write("var");
            return true;
        }
        wrote
    }

    /// Dart `_addSuperTypeProposals`.
    fn add_super_type_proposals(
        &mut self,
        ctx: &Ctx<'_>,
        ty: Option<TypeId>,
        added: &mut Vec<TypeId>,
    ) {
        let Some(ty) = ty else { return };
        if !matches!(ctx.ty(ty), TypeKind::Interface { .. }) {
            return;
        }
        if added.contains(&ty) {
            return;
        }
        added.push(ty);
        let display = dartr_element::type_display_string_with(ctx, ty, Default::default());
        self.add_suggestion(LinkedEditSuggestionKind::Type, &display);
        let supertype = ctx.superclass(ty);
        self.add_super_type_proposals(ctx, supertype, added);
        for i in ctx.interfaces(ty) {
            self.add_super_type_proposals(ctx, Some(i), added);
        }
    }

    /// Dart `_writeTypeIfCan`.
    fn write_type_if_can(
        &mut self,
        ctx: &Ctx<'_>,
        ty: TypeId,
        in_scope: &[EId<TypeParameterElement>],
        should_write_dynamic: bool,
        write_type_arguments: bool,
        prefix: Option<&str>,
    ) -> bool {
        let visible = self.visible_type(ctx, ty, in_scope);
        if !should_write_dynamic && matches!(ctx.ty(visible), TypeKind::Dynamic) {
            return false;
        }
        if !self.can_write_type_impl(ctx, visible, in_scope) {
            return false;
        }
        if let Some(prefix) = prefix {
            self.write(prefix);
        }
        let mut seen = Vec::new();
        self.write_type_impl(
            ctx,
            visible,
            in_scope,
            should_write_dynamic,
            write_type_arguments,
            &mut seen,
        );
        true
    }

    fn write_nullability(&mut self, ctx: &Ctx<'_>, ty: TypeId) {
        if nullability_of(ctx, ty) == Nullability::Question {
            self.write("?");
        }
    }

    /// Dart `_writeType`.
    fn write_type_impl(
        &mut self,
        ctx: &Ctx<'_>,
        ty: TypeId,
        in_scope: &[EId<TypeParameterElement>],
        should_write_dynamic: bool,
        write_type_arguments: bool,
        seen: &mut Vec<TypeId>,
    ) {
        seen.push(ty);
        let ty = self.visible_type(ctx, ty, in_scope);
        let ty = if matches!(ctx.ty(ty), TypeKind::Invalid | TypeKind::Unknown) {
            ctx.tp.object_question_type()
        } else {
            ty
        };
        match *ctx.ty(ty) {
            TypeKind::Dynamic => {
                if should_write_dynamic {
                    self.write("dynamic");
                }
                return;
            }
            TypeKind::Never(_) => {
                self.write("Never");
                self.write_nullability(ctx, ty);
                return;
            }
            TypeKind::Void => {
                self.write("void");
                return;
            }
            TypeKind::TypeParameter { param, .. } => {
                let name = ctx.element_name(param.raw()).unwrap_or("").to_string();
                self.write(&name);
                self.write_nullability(ctx, ty);
                return;
            }
            _ => {}
        }
        // An accessible alias, else the interface element.
        let alias = match *ctx.ty(ty) {
            TypeKind::Interface { alias, .. }
            | TypeKind::Record { alias, .. }
            | TypeKind::TypeParameter { alias, .. } => alias,
            TypeKind::Function(f) => f.alias,
            _ => None,
        };
        let alias = alias.filter(|a| {
            let element = ctx.alias(*a).element;
            dartr_typesystem::member::is_accessible_in(
                ctx,
                dartr_element::ElemRef::Base(element.raw()),
                self.library(),
            )
        });
        let element_and_args = match (alias, *ctx.ty(ty)) {
            (Some(a), _) => {
                let r = ctx.alias(a);
                Some((r.element.raw(), ctx.list(r.args).to_vec()))
            }
            (None, TypeKind::Interface { element, args, .. }) => {
                Some((element.raw(), ctx.list(args).to_vec()))
            }
            _ => None,
        };
        if let Some((element, args)) = element_and_args {
            self.write_library_reference(ctx, element);
            let name = ctx.element_name(element).unwrap_or("").to_string();
            self.write(&name);
            if write_type_arguments && !args.is_empty() {
                self.write("<");
                for (i, &argument) in args.iter().enumerate() {
                    let argument = self.visible_type(ctx, argument, in_scope);
                    if i != 0 {
                        self.write(", ");
                    }
                    if contains_element_and_arguments(ctx, seen, argument) {
                        self.write("dynamic");
                        continue;
                    }
                    let mut seen2 = seen.clone();
                    self.write_type_impl(
                        ctx,
                        argument,
                        in_scope,
                        true,
                        write_type_arguments,
                        &mut seen2,
                    );
                }
                self.write(">");
            }
            self.write_nullability(ctx, ty);
            return;
        }
        match *ctx.ty(ty) {
            TypeKind::Function(f) => {
                let mut scope: Vec<EId<TypeParameterElement>> = ctx.list(f.type_params).to_vec();
                scope.extend_from_slice(in_scope);
                self.write_type_impl(
                    ctx,
                    f.ret,
                    &scope,
                    should_write_dynamic,
                    write_type_arguments,
                    seen,
                );
                if should_write_dynamic || !matches!(ctx.ty(f.ret), TypeKind::Dynamic) {
                    self.write(" ");
                }
                self.write("Function");
                let type_params = ctx.list(f.type_params).to_vec();
                if !type_params.is_empty() {
                    self.write("<");
                    for (i, tp) in type_params.iter().enumerate() {
                        if i > 0 {
                            self.write(", ");
                        }
                        let name = ctx.element_name(tp.raw()).unwrap_or("").to_string();
                        self.write(&name);
                        if let Some(bound) = ctx.get(*tp).bound.get() {
                            self.write_type_if_can(
                                ctx,
                                bound,
                                &scope,
                                true,
                                write_type_arguments,
                                Some(" extends "),
                            );
                        }
                    }
                    self.write(">");
                }
                // Dart `writeFormalParameters` without names and defaults.
                self.write("(");
                let params = ctx.list(f.params).to_vec();
                let mut saw_named = false;
                let mut saw_positional = false;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        self.write(", ");
                    }
                    use dartr_element::ParameterKind as K;
                    match p.kind {
                        K::Named | K::NamedRequired if !saw_named => {
                            self.write("{");
                            saw_named = true;
                        }
                        K::Positional if !saw_positional => {
                            self.write("[");
                            saw_positional = true;
                        }
                        _ => {}
                    }
                    if p.kind == K::NamedRequired {
                        self.write("required ");
                    }
                    let had_type =
                        self.write_type_if_can(ctx, p.ty, &scope, true, write_type_arguments, None);
                    if matches!(p.kind, K::Named | K::NamedRequired) {
                        if let Some(n) = p.name {
                            if had_type {
                                self.write(" ");
                            }
                            let n = ctx.name_str(n).to_string();
                            self.write(&n);
                        }
                    }
                }
                if saw_named {
                    self.write("}");
                }
                if saw_positional {
                    self.write("]");
                }
                self.write(")");
                if f.nullability == Nullability::Question {
                    self.write("?");
                }
            }
            TypeKind::Record {
                positional, named, ..
            } => {
                self.write("(");
                let positional = ctx.list(positional).to_vec();
                let named = ctx.list(named).to_vec();
                let mut first = true;
                for &t in &positional {
                    if !first {
                        self.write(", ");
                    }
                    first = false;
                    self.write_type_impl(
                        ctx,
                        t,
                        in_scope,
                        should_write_dynamic,
                        write_type_arguments,
                        seen,
                    );
                }
                if positional.len() == 1 && named.is_empty() {
                    self.write(",");
                }
                if !named.is_empty() {
                    self.write(if first { "{" } else { ", {" });
                    let mut first = true;
                    for n in &named {
                        if !first {
                            self.write(", ");
                        }
                        first = false;
                        self.write_type_impl(
                            ctx,
                            n.ty,
                            in_scope,
                            should_write_dynamic,
                            write_type_arguments,
                            seen,
                        );
                        self.write(" ");
                        let name = ctx.name_str(n.name).to_string();
                        self.write(&name);
                    }
                    self.write("}");
                }
                self.write(")");
                self.write_nullability(ctx, ty);
            }
            _ => {}
        }
    }

    /// Dart `_writeLibraryReference`: imports the library of [element]
    /// when needed and writes the import prefix.
    fn write_library_reference(&mut self, ctx: &Ctx<'_>, element: ElementId) {
        if self.is_defined_locally(ctx, element) {
            return;
        }
        let Some(element_library) = dartr_resolver::error::support::library_of(ctx, element) else {
            return;
        };
        let element_uri = library_uri(ctx, element_library);
        let name = ctx.element_name(element).unwrap_or("").to_string();
        // Dart `_getImportElement`: an import whose namespace has the
        // element.
        let library = self.library();
        for import in existing_imports(ctx, library) {
            let Some(imported) = import.library else {
                continue;
            };
            let exported = super::producers::import_library::exported_element(ctx, imported, &name);
            let visible = exported.is_some_and(|e| {
                dartr_resolver::error::support::library_of(ctx, e).map(|l| library_uri(ctx, l))
                    == Some(element_uri.clone())
            }) && import.combinators.iter().all(|(is_show, names)| {
                if *is_show {
                    names.contains(&name)
                } else {
                    !names.contains(&name)
                }
            });
            if visible {
                if let Some(prefix) = import.prefix {
                    self.write(&format!("{prefix}."));
                }
                return;
            }
        }
        // A pending import of the element (Dart `_elementLibrariesToImport`)
        // or a new import.
        let shadowed = ctx
            .get(library)
            .public_namespace
            .try_get()
            .is_some_and(|ns| ns.defined_names.keys().any(|n| ctx.name_str(*n) == name));
        let prefix = if shadowed {
            Some(self.default_import_prefix(ctx))
        } else {
            None
        };
        self.file.import_library_impl(
            &element_uri,
            prefix.as_deref(),
            Some(&name),
            true,
            false,
            false,
            false,
        );
        if let Some(prefix) = prefix {
            self.write(&format!("{prefix}."));
        }
    }

    /// Dart `_defaultImportPrefixFor`.
    fn default_import_prefix(&self, ctx: &Ctx<'_>) -> String {
        let library = self.library();
        let mut existing: Vec<String> = Vec::new();
        if let Some(ns) = ctx.get(library).export_namespace.try_get() {
            existing.extend(
                ns.defined_names
                    .keys()
                    .map(|n| ctx.name_str(*n).to_string()),
            );
        }
        for import in existing_imports(ctx, library) {
            if let Some(p) = import.prefix {
                existing.push(p);
            }
        }
        let mut suffix = 0;
        loop {
            let prefix = format!("prefix{suffix}");
            if !existing.contains(&prefix) {
                return prefix;
            }
            suffix += 1;
        }
    }
}

/// Dart `Set<DartType>.containsElementAndArguments`.
fn contains_element_and_arguments(ctx: &Ctx<'_>, seen: &[TypeId], argument: TypeId) -> bool {
    for &t in seen {
        if t == argument {
            return true;
        }
        if let (
            TypeKind::Interface {
                element: e1,
                args: a1,
                ..
            },
            TypeKind::Interface {
                element: e2,
                args: a2,
                ..
            },
        ) = (*ctx.ty(t), *ctx.ty(argument))
        {
            if e1 == e2 && ctx.list(a1) == ctx.list(a2) {
                return true;
            }
        }
    }
    false
}

/// The element of an interface type, if any.
pub fn interface_element(ctx: &Ctx<'_>, ty: TypeId) -> Option<EId<InterfaceElement>> {
    match *ctx.ty(ty) {
        TypeKind::Interface { element, .. } => Some(element),
        _ => None,
    }
}

/// Dart `getCamelWords`.
pub fn get_camel_words(name: Option<&str>) -> Vec<String> {
    let Some(name) = name.filter(|n| !n.is_empty()) else {
        return Vec::new();
    };
    let chars: Vec<char> = name.chars().collect();
    let mut parts = Vec::new();
    let mut was_lower = false;
    let mut was_upper = false;
    let mut word_start = 0;
    for i in 0..chars.len() {
        let c = chars[i];
        let new_lower = c.is_ascii_lowercase();
        let new_upper = c.is_ascii_uppercase();
        // myWord
        // | ^
        if was_lower && new_upper {
            parts.push(chars[word_start..i].iter().collect::<String>());
            word_start = i;
        }
        // myHTTPRequest
        //       | ^
        if was_upper && new_upper && i + 1 < chars.len() && chars[i + 1].is_ascii_lowercase() {
            parts.push(chars[word_start..i].iter().collect::<String>());
            word_start = i;
        }
        was_lower = new_lower;
        was_upper = new_upper;
    }
    parts.push(chars[word_start..].iter().collect::<String>());
    parts
}

impl super::change_builder::FileEditBuilder<'_, '_> {
    /// Dart `DartFileEditBuilder.canWriteType` at [offset].
    pub fn can_write_type_at(&mut self, offset: u32, ty: TypeId) -> bool {
        let mut e = self.create_edit_builder(offset, 0);
        e.can_write_type(Some(ty), &[])
    }
}

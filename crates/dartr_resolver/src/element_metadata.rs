// Dart source: pkg/analyzer/lib/src/dart/element/element.dart
// (ElementAnnotationImpl: element, is* getters, deprecationKind;
// MetadataImpl: has* getters; FragmentImpl.elementMetadata),
// pkg/analyzer/lib/src/dart/element/annotation_target.dart,
// pkg/analyzer/lib/src/utilities/extensions/element.dart (isInternal,
// isProtected, isVisibleForTesting),
// pkg/analyzer/lib/src/dart/element/extensions.dart (hasOrInheritsDoNotStore),
// pkg/analyzer/lib/src/workspace/pub.dart (PubPackage.contains,
// sourceIsInPublicApi, isInTestDirectory)

//! The annotations of elements (Dart `Element.metadata`) and the `has*`
//! flags that the verifiers read (`@Deprecated`, `@override` and the
//! annotations of `package:meta`).
//!
//! Differences:
//! - Dart resolves the annotations of the linked elements while linking
//!   (`_resolveMetadata`, an `AstResolver` over each annotation). The linker
//!   of this port keeps unresolved copies in the cycle's `ConstExprs`
//!   ([`dartr_element::ElementStore::const_ast`]), so
//!   [`annotation_element`] resolves the name of an annotation when it is
//!   read: a lookup in the scope of the library fragment of the annotation
//!   (declarations of the library, then the imports, by prefix), then the
//!   constructor or static getter. This is the element that Dart's
//!   `AnnotationResolver` finds for a valid annotation; type arguments and
//!   the arguments are not resolved.
//! - Dart reads the values of annotations with `computeConstantValue()`.
//!   The constant evaluation of annotations of other libraries is not
//!   available here, so the values that the verifiers read (the message of
//!   `@Deprecated`, `TargetKind`s of `@Target`, `parameterDefined` of
//!   `@UseResult.unless`) come from the syntax of the arguments: string
//!   literals (also adjacent) and `TargetKind.x` identifiers. A value that
//!   is not a literal (a constant reference) is treated as absent.
//! - The flags of a [`Metadata`] are cached in
//!   [`Metadata::metadata_flags`] (Dart `MetadataImpl._metadataFlags2`
//!   caches only `hasDeprecated` and `hasOverride`).

use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

use dartr_ast::{
    AdjacentStrings, Annotation, Ast, Expression, Id, Identifier, NamedArgument, NodeId,
    PrefixedIdentifier, SetOrMapLiteral, SimpleIdentifier, SimpleStringLiteral,
};
use dartr_element::{
    Ctx, DirectiveUri, EId, ElementId, FId, FragmentId, InterfaceElement, LibraryElement,
    LibraryFragment, Metadata, NamespaceCombinator, PrefixElement, ResolutionTables, StoreId, Tag,
    TypeAliasElement, TypeKind,
};
use dartr_typesystem::{TypeExt, lookup};
use indexmap::IndexMap;

/// The `has*` bits of a [`Metadata`] (one per `ElementAnnotationImpl.is*`
/// getter that the verifiers read).
pub mod flags {
    /// The flags are computed (Dart `MetadataImpl._isReady`).
    pub const READY: u32 = 1 << 0;
    pub const DEPRECATED: u32 = 1 << 1;
    pub const OVERRIDE: u32 = 1 << 2;
    pub const ALWAYS_THROWS: u32 = 1 << 3;
    pub const AWAIT_NOT_REQUIRED: u32 = 1 << 4;
    pub const DO_NOT_STORE: u32 = 1 << 5;
    pub const DO_NOT_SUBMIT: u32 = 1 << 6;
    pub const EXPERIMENTAL: u32 = 1 << 7;
    pub const FACTORY: u32 = 1 << 8;
    pub const IMMUTABLE: u32 = 1 << 9;
    pub const INTERNAL: u32 = 1 << 10;
    pub const IS_TEST: u32 = 1 << 11;
    pub const IS_TEST_GROUP: u32 = 1 << 12;
    pub const JS: u32 = 1 << 13;
    pub const LITERAL: u32 = 1 << 14;
    pub const MUST_BE_CONST: u32 = 1 << 15;
    pub const MUST_BE_OVERRIDDEN: u32 = 1 << 16;
    pub const MUST_CALL_SUPER: u32 = 1 << 17;
    pub const NON_VIRTUAL: u32 = 1 << 18;
    pub const OPTIONAL_TYPE_ARGS: u32 = 1 << 19;
    pub const PROTECTED: u32 = 1 << 20;
    pub const REDECLARE: u32 = 1 << 21;
    pub const REOPEN: u32 = 1 << 22;
    pub const REQUIRED: u32 = 1 << 23;
    pub const SEALED: u32 = 1 << 24;
    pub const USE_RESULT: u32 = 1 << 25;
    pub const VISIBLE_FOR_OVERRIDING: u32 = 1 << 26;
    pub const VISIBLE_FOR_TEMPLATE: u32 = 1 << 27;
    pub const VISIBLE_FOR_TESTING: u32 = 1 << 28;
    pub const VISIBLE_OUTSIDE_TEMPLATE: u32 = 1 << 29;
    pub const WIDGET_FACTORY: u32 = 1 << 30;
    /// Dart `isTarget` (`@Target` of `package:meta/meta_meta.dart`).
    pub const TARGET: u32 = 1 << 31;
}

/// The AST of the unit under analysis and its resolution, for the
/// annotations of local elements (their `ConstExprId`s point into the unit)
/// and of the annotation nodes of the unit.
#[derive(Clone, Copy)]
pub struct UnitAst<'u> {
    pub ast: &'u Ast,
    pub tables: &'u ResolutionTables,
}

/// One annotation: Dart `ElementAnnotationImpl` (its `annotationAst`, its
/// `libraryFragment` and its resolved `element`).
#[derive(Clone, Copy)]
pub struct AnnotationRef<'a> {
    pub ast: &'a Ast,
    pub node: Id<Annotation>,
    /// Dart `libraryFragment`.
    pub fragment: FId<LibraryFragment>,
    /// Dart `element`.
    pub element: Option<ElementId>,
}

impl<'a> AnnotationRef<'a> {
    /// The annotation [node] of the unit under analysis, declared in
    /// [fragment] (Dart `node.elementAnnotation`): the element that the
    /// resolver recorded, else the syntactic resolution.
    pub fn of_node(
        ctx: &Ctx<'_>,
        unit: UnitAst<'a>,
        node: Id<Annotation>,
        fragment: FId<LibraryFragment>,
    ) -> AnnotationRef<'a> {
        let recorded = unit
            .tables
            .element
            .get(node.raw())
            .map(|&e| dartr_typesystem::member::base_element(ctx, e));
        let element = recorded.or_else(|| annotation_element(ctx, unit.ast, node, fragment));
        AnnotationRef {
            ast: unit.ast,
            node,
            fragment,
            element,
        }
    }

    /// The flag bit of this annotation (one of [`flags`], or 0).
    pub fn kind(&self, ctx: &Ctx<'_>) -> u32 {
        annotation_kind(ctx, self.element)
    }

    pub fn is(&self, ctx: &Ctx<'_>, flag: u32) -> bool {
        self.kind(ctx) & flag != 0
    }

    /// Dart `deprecationKind`: the name of the `Deprecated` constructor
    /// (`use` for the unnamed one and the `deprecated` getter), `None` if
    /// this is not a deprecation annotation.
    pub fn deprecation_kind(&self, ctx: &Ctx<'_>) -> Option<&'static str> {
        if !self.is(ctx, flags::DEPRECATED) {
            return None;
        }
        let element = self.element?;
        if element.tag() != Tag::Constructor {
            return Some("use");
        }
        Some(match ctx.element_name(element) {
            Some("extend") => "extend",
            Some("implement") => "implement",
            Some("subclass") => "subclass",
            Some("instantiate") => "instantiate",
            Some("mixin") => "mixin",
            Some("optional") => "optional",
            _ => "use",
        })
    }

    /// The positional argument [index] or the named argument [name]
    /// (`None`: absent).
    pub fn argument(&self, index: Option<usize>, name: Option<&str>) -> Option<Id<Expression>> {
        let ast = self.ast;
        let arguments = ast[self.node].arguments?;
        let mut positional = 0;
        for &argument in ast.list(ast[arguments].arguments) {
            if let Some(named) = ast.cast::<NamedArgument>(argument) {
                if name == Some(ast.tokens.lexeme(ast[named].name)) {
                    return Some(ast[named].argument_expression);
                }
            } else {
                if index == Some(positional) {
                    return ast.cast::<Expression>(argument);
                }
                positional += 1;
            }
        }
        None
    }

    /// The string value of the positional argument [index] or of the named
    /// argument [name] (Dart `computeConstantValue()?.getField(field)
    /// ?.toStringValue()` for a field that the constructor initializes from
    /// that parameter).
    pub fn string_argument(&self, index: Option<usize>, name: Option<&str>) -> Option<String> {
        let expression = self.argument(index, name)?;
        string_value(self.ast, expression.raw())
    }
}

/// The string value of a string literal without interpolation.
pub fn string_value(ast: &Ast, node: NodeId) -> Option<String> {
    if let Some(s) = ast.cast::<SimpleStringLiteral>(node) {
        return Some(ast[s].value.to_string());
    }
    if let Some(a) = ast.cast::<AdjacentStrings>(node) {
        let mut buffer = String::new();
        for &s in ast.list(ast[a].strings) {
            buffer.push_str(&string_value(ast, s.raw())?);
        }
        return Some(buffer);
    }
    None
}

/// The annotations of the fragments of [element] (Dart
/// `element.metadata.annotations`, `FragmentImpl.elementMetadata`).
pub fn element_annotations<'a>(
    ctx: &Ctx<'a>,
    element: ElementId,
    unit: Option<UnitAst<'a>>,
) -> Vec<AnnotationRef<'a>> {
    let mut result = Vec::new();
    for_each_metadata(ctx, element, |metadata, store| {
        for annotation in &metadata.annotations {
            if let Some(r) = annotation_ref(ctx, annotation, store, unit) {
                result.push(r);
            }
        }
        false
    });
    result
}

/// Dart `element.metadata.hasX` for the bits of [flag] (any of them).
pub fn element_has(
    ctx: &Ctx<'_>,
    element: ElementId,
    flag: u32,
    unit: Option<UnitAst<'_>>,
) -> bool {
    let mut found = false;
    for_each_metadata(ctx, element, |metadata, store| {
        if metadata_flags(ctx, metadata, store, unit) & flag != 0 {
            found = true;
        }
        found
    });
    found
}

/// The metadata of the fragments of [element] (the library metadata of a
/// library), until [f] returns `true`.
fn for_each_metadata<'a>(
    ctx: &Ctx<'a>,
    element: ElementId,
    mut f: impl FnMut(&'a Metadata, StoreId) -> bool,
) {
    match element.tag() {
        Tag::Dynamic | Tag::Never | Tag::MultiplyDefined => {}
        Tag::Library => {
            if let Some(library) = element.cast::<LibraryElement>() {
                f(&ctx.get(library).metadata, element.store());
            }
        }
        _ => {
            let Some(data) = ctx.element_data(element) else {
                return;
            };
            let mut fragment: Option<FragmentId> = Some(data.first_fragment);
            while let Some(id) = fragment {
                let Some(fd) = ctx.fragment_data(id) else {
                    return;
                };
                if !fd.metadata.annotations.is_empty() && f(&fd.metadata, id.store()) {
                    return;
                }
                fragment = fd.next_fragment;
            }
        }
    }
}

/// The flags of [metadata] (of a fragment of the store [store]), cached.
pub fn metadata_flags(
    ctx: &Ctx<'_>,
    metadata: &Metadata,
    store: StoreId,
    unit: Option<UnitAst<'_>>,
) -> u32 {
    if metadata.annotations.is_empty() {
        return 0;
    }
    if let Some(&bits) = metadata.metadata_flags.try_get() {
        return bits;
    }
    let mut bits = flags::READY;
    let mut complete = true;
    for annotation in &metadata.annotations {
        match annotation_ref(ctx, annotation, store, unit) {
            Some(r) => bits |= r.kind(ctx),
            None => complete = false,
        }
    }
    if complete {
        *metadata.metadata_flags.get_or_init(|| bits)
    } else {
        bits
    }
}

/// The [AnnotationRef] of an `ElementAnnotation` of a fragment of [store].
fn annotation_ref<'a>(
    ctx: &Ctx<'a>,
    annotation: &dartr_element::ElementAnnotation,
    store: StoreId,
    unit: Option<UnitAst<'a>>,
) -> Option<AnnotationRef<'a>> {
    let node_id = annotation.annotation_ast.0;
    if store.is_local() {
        let unit = unit?;
        let node = unit.ast.cast::<Annotation>(node_id)?;
        return Some(AnnotationRef::of_node(
            ctx,
            unit,
            node,
            annotation.library_fragment,
        ));
    }
    let ast: &'a Ast = ctx.store(store).const_ast.try_get()?.ast();
    let node = ast.cast::<Annotation>(node_id)?;
    Some(AnnotationRef {
        ast,
        node,
        fragment: annotation.library_fragment,
        element: annotation_element(ctx, ast, node, annotation.library_fragment),
    })
}

/// The flag bit of an annotation whose element is [element] (Dart
/// `ElementAnnotationImpl.is*`).
pub fn annotation_kind(ctx: &Ctx<'_>, element: Option<ElementId>) -> u32 {
    let Some(element) = element else {
        return 0;
    };
    let Some(data) = ctx.element_data(element) else {
        return 0;
    };
    let Some(library) = data.library else {
        return 0;
    };
    let library_name = ctx.element_name(library.raw()).unwrap_or("");
    let library_uri = ctx.library_uri(library);
    let name = ctx.element_name(element).unwrap_or("");
    match element.tag() {
        Tag::Constructor => {
            let class_name = data
                .enclosing
                .and_then(|c| ctx.element_name(c))
                .unwrap_or("");
            match (library_name, class_name) {
                ("dart.core", "Deprecated") => flags::DEPRECATED,
                ("meta", "Immutable") => flags::IMMUTABLE,
                ("meta", "Required") => flags::REQUIRED,
                ("meta", "UseResult") => flags::USE_RESULT,
                ("meta_meta", "Target") => flags::TARGET,
                ("_js_annotations", "JS") => flags::JS,
                _ if class_name == "JS" && library_uri == "dart:js_interop" => flags::JS,
                _ => 0,
            }
        }
        Tag::Getter => match library_name {
            "dart.core" => match name {
                "deprecated" => flags::DEPRECATED,
                "override" => flags::OVERRIDE,
                _ => 0,
            },
            "meta" => match name {
                "alwaysThrows" => flags::ALWAYS_THROWS,
                "awaitNotRequired" => flags::AWAIT_NOT_REQUIRED,
                "doNotStore" => flags::DO_NOT_STORE,
                "doNotSubmit" => flags::DO_NOT_SUBMIT,
                "experimental" => flags::EXPERIMENTAL,
                "factory" => flags::FACTORY,
                "immutable" => flags::IMMUTABLE,
                "internal" => flags::INTERNAL,
                "isTest" => flags::IS_TEST,
                "isTestGroup" => flags::IS_TEST_GROUP,
                "literal" => flags::LITERAL,
                "mustBeConst" => flags::MUST_BE_CONST,
                "mustBeOverridden" => flags::MUST_BE_OVERRIDDEN,
                "mustCallSuper" => flags::MUST_CALL_SUPER,
                "nonVirtual" => flags::NON_VIRTUAL,
                "optionalTypeArgs" => flags::OPTIONAL_TYPE_ARGS,
                "protected" => flags::PROTECTED,
                "redeclare" => flags::REDECLARE,
                "reopen" => flags::REOPEN,
                "required" => flags::REQUIRED,
                "sealed" => flags::SEALED,
                "useResult" => flags::USE_RESULT,
                "visibleForOverriding" => flags::VISIBLE_FOR_OVERRIDING,
                "visibleForTesting" => flags::VISIBLE_FOR_TESTING,
                _ => 0,
            },
            "angular.meta" => match name {
                "visibleForTemplate" => flags::VISIBLE_FOR_TEMPLATE,
                "visibleOutsideTemplate" => flags::VISIBLE_OUTSIDE_TEMPLATE,
                _ => 0,
            },
            _ if name == "widgetFactory"
                && library_uri == "package:flutter/src/widgets/widget_inspector.dart" =>
            {
                flags::WIDGET_FACTORY
            }
            _ => 0,
        },
        _ => 0,
    }
}

// ------------------------------------------------------------------ resolution

/// A name in the scope of a library fragment.
enum ScopeName {
    Element(ElementId),
    Prefix(EId<PrefixElement>),
}

/// The element of the annotation [node] in [fragment] (Dart
/// `AnnotationResolver`, for a valid annotation): the getter of a constant
/// variable, or the constructor of a class (also through a type alias), or
/// a static getter of a class.
pub fn annotation_element(
    ctx: &Ctx<'_>,
    ast: &Ast,
    node: Id<Annotation>,
    fragment: FId<LibraryFragment>,
) -> Option<ElementId> {
    let a = &ast[node];
    let constructor_name = a.constructor_name.map(|n| identifier_text(ast, n));
    let has_arguments = a.arguments.is_some();
    let name: Id<Identifier> = a.name;
    if let Some(simple) = ast.cast::<SimpleIdentifier>(name) {
        let target = match scope_lookup(ctx, fragment, identifier_text(ast, simple))? {
            ScopeName::Element(e) => e,
            ScopeName::Prefix(_) => return None,
        };
        return resolve_target(ctx, target, constructor_name, has_arguments, fragment);
    }
    let prefixed = ast.cast::<PrefixedIdentifier>(name)?;
    let prefix = identifier_text(ast, ast[prefixed].prefix);
    let identifier = identifier_text(ast, ast[prefixed].identifier);
    match scope_lookup(ctx, fragment, prefix)? {
        ScopeName::Prefix(prefix) => {
            let target = prefix_lookup(ctx, fragment, prefix, identifier)?;
            resolve_target(ctx, target, constructor_name, has_arguments, fragment)
        }
        ScopeName::Element(e) => {
            // `@A.name(...)` (a named constructor) or `@A.name` (a static
            // getter).
            if constructor_name.is_some() {
                return None;
            }
            let interface = interface_of(ctx, e)?;
            if has_arguments {
                lookup::get_named_constructor(ctx, interface, identifier).map(|c| c.raw())
            } else {
                static_getter(ctx, interface, identifier, fragment)
            }
        }
    }
}

fn identifier_text<'t>(ast: &'t Ast, node: Id<SimpleIdentifier>) -> &'t str {
    ast.tokens.lexeme(ast[node].token)
}

/// The element of `@target`, `@target.constructorName(...)` or
/// `@target(...)`.
fn resolve_target(
    ctx: &Ctx<'_>,
    target: ElementId,
    constructor_name: Option<&str>,
    has_arguments: bool,
    fragment: FId<LibraryFragment>,
) -> Option<ElementId> {
    if target.tag() == Tag::Getter {
        return (constructor_name.is_none() && !has_arguments).then_some(target);
    }
    let interface = interface_of(ctx, target)?;
    if has_arguments {
        return lookup::get_named_constructor(ctx, interface, constructor_name.unwrap_or("new"))
            .map(|c| c.raw());
    }
    let name = constructor_name?;
    static_getter(ctx, interface, name, fragment)
}

fn static_getter(
    ctx: &Ctx<'_>,
    interface: EId<InterfaceElement>,
    name: &str,
    fragment: FId<LibraryFragment>,
) -> Option<ElementId> {
    let library = ctx.fragment(fragment).library;
    lookup::look_up_static_getter(ctx, interface, name, library).map(|g| g.raw())
}

/// The interface element of a class-like element or of a type alias of an
/// interface type.
fn interface_of(ctx: &Ctx<'_>, element: ElementId) -> Option<EId<InterfaceElement>> {
    if let Some(i) = element.cast::<InterfaceElement>() {
        return Some(i);
    }
    let alias = element.cast::<TypeAliasElement>()?;
    let aliased = ctx.get(alias).aliased_type.get()?;
    match *ctx.ty(aliased) {
        TypeKind::Interface { element, .. } => Some(element),
        _ => None,
    }
}

/// Dart `LibraryFragmentScope.lookup(name).getter` for a fragment of a
/// linked library.
fn scope_lookup(ctx: &Ctx<'_>, fragment: FId<LibraryFragment>, name: &str) -> Option<ScopeName> {
    let f = ctx.fragment(fragment);
    if let Some(e) = library_declaration(ctx, f.library, name) {
        return Some(ScopeName::Element(e));
    }
    let mut current = Some(fragment);
    while let Some(id) = current {
        let fd = ctx.fragment(id);
        if let Some(&prefix) = fd.library_import_prefixes_by_id.get(&ctx.name(name)) {
            return Some(ScopeName::Prefix(prefix));
        }
        if let Some(e) = import_lookup(ctx, id, None, name) {
            return Some(ScopeName::Element(e));
        }
        current = parent_fragment(ctx, id);
    }
    None
}

/// Dart `PrefixScope.lookup` of the imports with [prefix].
fn prefix_lookup(
    ctx: &Ctx<'_>,
    fragment: FId<LibraryFragment>,
    prefix: EId<PrefixElement>,
    name: &str,
) -> Option<ElementId> {
    let mut current = Some(fragment);
    while let Some(id) = current {
        if let Some(e) = import_lookup(ctx, id, Some(prefix), name) {
            return Some(e);
        }
        current = parent_fragment(ctx, id);
    }
    None
}

/// The element named [name] in the export namespaces of the imports of
/// [fragment] with [prefix]. A conflict between an SDK and a non-SDK
/// element prefers the non-SDK one; other conflicts have no element.
fn import_lookup(
    ctx: &Ctx<'_>,
    fragment: FId<LibraryFragment>,
    prefix: Option<EId<PrefixElement>>,
    name: &str,
) -> Option<ElementId> {
    let key = ctx.name(name);
    let mut found: Option<ElementId> = None;
    for import in &ctx.fragment(fragment).library_imports {
        let DirectiveUri::Library { library, .. } = &import.directive.uri else {
            continue;
        };
        let import_prefix = import
            .prefix
            .and_then(|p| ctx.fragment(p).element.try_get().copied());
        if import_prefix != prefix.map(|p| p.raw()) {
            continue;
        }
        if !combinators_allow(ctx, &import.combinators, name) {
            continue;
        }
        let Some(namespace) = ctx.get(*library).export_namespace.try_get() else {
            continue;
        };
        let Some(&element) = namespace.defined_names.get(&key) else {
            continue;
        };
        match found {
            None => found = Some(element),
            Some(existing) if existing == element => {}
            Some(existing) => {
                let existing_sdk = is_sdk_element(ctx, existing);
                let element_sdk = is_sdk_element(ctx, element);
                if existing_sdk && !element_sdk {
                    found = Some(element);
                } else if !existing_sdk && element_sdk {
                } else {
                    return None;
                }
            }
        }
    }
    found
}

fn is_sdk_element(ctx: &Ctx<'_>, element: ElementId) -> bool {
    ctx.element_library_uri(element)
        .is_some_and(|uri| uri.starts_with("dart:"))
}

fn combinators_allow(ctx: &Ctx<'_>, combinators: &[NamespaceCombinator], name: &str) -> bool {
    let matches = |names: &[dartr_element::Name]| names.iter().any(|&n| ctx.name_str(n) == name);
    for c in combinators {
        match c {
            NamespaceCombinator::Show { shown_names, .. } => {
                if !matches(shown_names) {
                    return false;
                }
            }
            NamespaceCombinator::Hide { hidden_names, .. } => {
                if matches(hidden_names) {
                    return false;
                }
            }
        }
    }
    true
}

/// The unit that includes [fragment] with a `part` directive.
fn parent_fragment(ctx: &Ctx<'_>, fragment: FId<LibraryFragment>) -> Option<FId<LibraryFragment>> {
    let f = ctx.fragment(fragment);
    if let Some(parent) = f
        .enclosing_fragment
        .and_then(|p| p.cast::<LibraryFragment>())
    {
        return Some(parent);
    }
    let library = ctx.get(f.library);
    let mut stack = vec![library.first_fragment()];
    while let Some(unit) = stack.pop() {
        for part in &ctx.fragment(unit).parts {
            if let DirectiveUri::Unit {
                library_fragment, ..
            } = &part.directive.uri
            {
                if *library_fragment == fragment {
                    return Some(unit);
                }
                stack.push(*library_fragment);
            }
        }
    }
    None
}

/// The top-level declaration (getter, class, function, ...) named [name]
/// of [library] (Dart `LibraryDeclarations`).
fn library_declaration(
    ctx: &Ctx<'_>,
    library: EId<LibraryElement>,
    name: &str,
) -> Option<ElementId> {
    let l = ctx.get(library);
    let named = |e: ElementId| ctx.element_name(e) == Some(name);
    l.getters
        .iter()
        .map(|e| e.raw())
        .chain(l.enums.iter().map(|e| e.raw()))
        .chain(l.extensions.iter().map(|e| e.raw()))
        .chain(l.extension_types.iter().map(|e| e.raw()))
        .chain(l.top_level_functions.iter().map(|e| e.raw()))
        .chain(l.type_aliases.iter().map(|e| e.raw()))
        .chain(l.mixins.iter().map(|e| e.raw()))
        .chain(l.classes.iter().map(|e| e.raw()))
        .find(|&e| named(e))
}

// ------------------------------------------------------------------ element extensions

/// Dart `Element.isInternal` (utilities/extensions/element.dart).
pub fn is_internal(ctx: &Ctx<'_>, element: ElementId, unit: Option<UnitAst<'_>>) -> bool {
    if element_has(ctx, element, flags::INTERNAL, unit) {
        return true;
    }
    if let Some(variable) = accessor_variable(ctx, element) {
        return element_has(ctx, variable, flags::INTERNAL, unit);
    }
    false
}

/// Dart `Element.isProtected`.
pub fn is_protected(ctx: &Ctx<'_>, element: ElementId, unit: Option<UnitAst<'_>>) -> bool {
    let enclosing_is_interface = ctx
        .element_data(element)
        .and_then(|d| d.enclosing)
        .is_some_and(|e| e.is::<InterfaceElement>());
    if matches!(element.tag(), Tag::Getter | Tag::Setter) && enclosing_is_interface {
        if element_has(ctx, element, flags::PROTECTED, unit) {
            return true;
        }
        if let Some(variable) = accessor_variable(ctx, element)
            && element_has(ctx, variable, flags::PROTECTED, unit)
        {
            return true;
        }
    }
    element.tag() == Tag::Method
        && enclosing_is_interface
        && element_has(ctx, element, flags::PROTECTED, unit)
}

/// Dart `Element.isVisibleForTesting`.
pub fn is_visible_for_testing(
    ctx: &Ctx<'_>,
    element: ElementId,
    unit: Option<UnitAst<'_>>,
) -> bool {
    if element_has(ctx, element, flags::VISIBLE_FOR_TESTING, unit) {
        return true;
    }
    if let Some(variable) = accessor_variable(ctx, element) {
        return element_has(ctx, variable, flags::VISIBLE_FOR_TESTING, unit);
    }
    false
}

/// Dart `Element.hasOrInheritsDoNotStore` (dart/element/extensions.dart).
pub fn has_or_inherits_do_not_store(
    ctx: &Ctx<'_>,
    element: ElementId,
    unit: Option<UnitAst<'_>>,
) -> bool {
    if element_has(ctx, element, flags::DO_NOT_STORE, unit) {
        return true;
    }
    let mut ancestor = ctx.element_data(element).and_then(|d| d.enclosing);
    if let Some(a) = ancestor
        && matches!(
            a.tag(),
            Tag::Class | Tag::Enum | Tag::Mixin | Tag::ExtensionType | Tag::Extension
        )
    {
        if element_has(ctx, a, flags::DO_NOT_STORE, unit) {
            return true;
        }
        ancestor = ctx.element_data(a).and_then(|d| d.enclosing);
    }
    ancestor
        .is_some_and(|a| a.tag() == Tag::Library && element_has(ctx, a, flags::DO_NOT_STORE, unit))
}

/// Dart `Element.isDeprecatedWithKind(kind)`.
pub fn is_deprecated_with_kind(
    ctx: &Ctx<'_>,
    element: ElementId,
    kind: &str,
    unit: Option<UnitAst<'_>>,
) -> bool {
    if !element_has(ctx, element, flags::DEPRECATED, unit) {
        return false;
    }
    element_annotations(ctx, element, unit)
        .iter()
        .any(|a| a.deprecation_kind(ctx) == Some(kind))
}

/// The variable of an accessor whose origin is a variable (Dart
/// `element is PropertyAccessorElement && element.isOriginVariable`, then
/// `element.variable`).
pub fn accessor_variable(ctx: &Ctx<'_>, element: ElementId) -> Option<ElementId> {
    if !matches!(element.tag(), Tag::Getter | Tag::Setter) {
        return None;
    }
    let accessor = element.cast::<dartr_element::PropertyAccessorElement>()?;
    if !is_origin_variable(ctx, element) {
        return None;
    }
    ctx.property_accessor(accessor)
        .variable
        .get()
        .map(|v| v.raw())
}

/// Dart `PropertyAccessorElement.variable` (also for an accessor that is
/// not synthetic).
pub fn accessor_variable_any(ctx: &Ctx<'_>, element: ElementId) -> Option<ElementId> {
    let accessor = element.cast::<dartr_element::PropertyAccessorElement>()?;
    ctx.property_accessor(accessor)
        .variable
        .get()
        .map(|v| v.raw())
}

/// Dart `PropertyAccessorElement.isOriginVariable`: a synthetic accessor of
/// a variable.
pub fn is_origin_variable(ctx: &Ctx<'_>, element: ElementId) -> bool {
    ctx.element_data(element)
        .and_then(|d| ctx.fragment_data(d.first_fragment))
        .is_some_and(|f| {
            f.flags
                .has(dartr_element::FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE)
        })
}

// ------------------------------------------------------------------ target kinds

/// Dart `TargetKind` (package:meta/meta_meta.dart).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum TargetKind {
    ClassType,
    Constructor,
    Directive,
    EnumType,
    EnumValue,
    ExportDirective,
    Extension,
    ExtensionType,
    Field,
    Function,
    ImportDirective,
    Getter,
    Library,
    Method,
    MixinType,
    OptionalParameter,
    OverridableMember,
    Parameter,
    PartOfDirective,
    Setter,
    TopLevelVariable,
    Type,
    TypedefType,
    TypeParameter,
}

impl TargetKind {
    fn from_name(name: &str) -> Option<TargetKind> {
        Some(match name {
            "classType" => TargetKind::ClassType,
            "constructor" => TargetKind::Constructor,
            "directive" => TargetKind::Directive,
            "enumType" => TargetKind::EnumType,
            "enumValue" => TargetKind::EnumValue,
            "exportDirective" => TargetKind::ExportDirective,
            "extension" => TargetKind::Extension,
            "extensionType" => TargetKind::ExtensionType,
            "field" => TargetKind::Field,
            "function" => TargetKind::Function,
            "importDirective" => TargetKind::ImportDirective,
            "getter" => TargetKind::Getter,
            "library" => TargetKind::Library,
            "method" => TargetKind::Method,
            "mixinType" => TargetKind::MixinType,
            "optionalParameter" => TargetKind::OptionalParameter,
            "overridableMember" => TargetKind::OverridableMember,
            "parameter" => TargetKind::Parameter,
            "partOfDirective" => TargetKind::PartOfDirective,
            "setter" => TargetKind::Setter,
            "topLevelVariable" => TargetKind::TopLevelVariable,
            "type" => TargetKind::Type,
            "typedefType" => TargetKind::TypedefType,
            "typeParameter" => TargetKind::TypeParameter,
            _ => return None,
        })
    }

    /// Dart `TargetKind.displayString`.
    pub fn display_string(self) -> &'static str {
        match self {
            TargetKind::ClassType => "classes",
            TargetKind::Constructor => "constructors",
            TargetKind::Directive => "directives",
            TargetKind::EnumType => "enums",
            TargetKind::EnumValue => "enum values",
            TargetKind::ExportDirective => "export directives",
            TargetKind::Extension => "extensions",
            TargetKind::ExtensionType => "extension types",
            TargetKind::Field => "fields",
            TargetKind::Function => "top-level functions",
            TargetKind::ImportDirective => "import directives",
            TargetKind::Getter => "getters",
            TargetKind::Library => "libraries",
            TargetKind::Method => "methods",
            TargetKind::MixinType => "mixins",
            TargetKind::OptionalParameter => "optional parameters",
            TargetKind::OverridableMember => "overridable members",
            TargetKind::Parameter => "parameters",
            TargetKind::PartOfDirective => "part of directives",
            TargetKind::Setter => "setters",
            TargetKind::TopLevelVariable => "top-level variables",
            TargetKind::Type => "types (classes, enums, mixins, or typedefs)",
            TargetKind::TypedefType => "typedefs",
            TargetKind::TypeParameter => "type parameters",
        }
    }
}

/// Dart `ElementAnnotation.targetKinds` (annotation_target.dart): the
/// kinds of `@Target` on the class of the annotation, `None` if unknown.
pub fn target_kinds(ctx: &Ctx<'_>, annotation: &AnnotationRef<'_>) -> Option<Vec<TargetKind>> {
    if annotation.is(ctx, flags::OVERRIDE) {
        return Some(vec![
            TargetKind::Field,
            TargetKind::Getter,
            TargetKind::Method,
            TargetKind::Setter,
        ]);
    }
    let element = annotation.element?;
    let interface = match element.tag() {
        Tag::Getter => {
            let ty = dartr_typesystem::member::return_type(ctx, element.into());
            match *ctx.ty(ty) {
                TypeKind::Interface { element, .. } => element,
                _ => return None,
            }
        }
        Tag::Constructor => ctx
            .element_data(element)?
            .enclosing?
            .cast::<InterfaceElement>()?,
        _ => return None,
    };
    for target in element_annotations(ctx, interface.raw(), None) {
        if target.is(ctx, flags::TARGET) {
            let kinds = target.argument(Some(0), Some("kinds"))?;
            let set = target.ast.cast::<SetOrMapLiteral>(kinds)?;
            let mut result = Vec::new();
            for &e in target.ast.list(target.ast[set].elements) {
                let name = if let Some(p) = target.ast.cast::<PrefixedIdentifier>(e) {
                    identifier_text(target.ast, target.ast[p].identifier)
                } else if let Some(s) = target.ast.cast::<SimpleIdentifier>(e) {
                    identifier_text(target.ast, s)
                } else {
                    return None;
                };
                if let Some(kind) = TargetKind::from_name(name)
                    && !result.contains(&kind)
                {
                    result.push(kind);
                }
            }
            return Some(result);
        }
    }
    None
}

// ------------------------------------------------------------------ workspace

/// Dart `WorkspacePackageImpl` of a pub package (the folder with the
/// nearest `pubspec.yaml`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspacePackage {
    pub root: PathBuf,
    /// The `name:` of the pubspec.
    pub name: Option<String>,
}

static PACKAGES: LazyLock<Mutex<IndexMap<PathBuf, Option<Arc<WorkspacePackage>>>>> =
    LazyLock::new(|| Mutex::new(IndexMap::new()));

impl WorkspacePackage {
    /// The package of the file [path] (Dart `Workspace.findPackageFor`).
    pub fn find_for(path: &str) -> Option<Arc<WorkspacePackage>> {
        let dir = Path::new(path).parent()?.to_path_buf();
        if let Ok(cache) = PACKAGES.lock()
            && let Some(p) = cache.get(&dir)
        {
            return p.clone();
        }
        let found = dir
            .ancestors()
            .find(|d| d.join("pubspec.yaml").is_file())
            .map(|root| {
                let name = std::fs::read_to_string(root.join("pubspec.yaml"))
                    .ok()
                    .and_then(|text| {
                        text.lines().find_map(|l| {
                            l.strip_prefix("name:")
                                .map(|n| n.trim().trim_matches(['\'', '"']).to_string())
                        })
                    });
                Arc::new(WorkspacePackage {
                    root: root.to_path_buf(),
                    name,
                })
            });
        if let Ok(mut cache) = PACKAGES.lock() {
            cache.insert(dir, found.clone());
        }
        found
    }

    /// Dart `PubPackage.contains(source)` of a source with [uri] and
    /// [path].
    pub fn contains(&self, uri: &str, path: &str) -> bool {
        if let Some(rest) = uri.strip_prefix("package:") {
            let package_name = rest.split('/').next().unwrap_or("");
            return self.name.as_deref() == Some(package_name);
        }
        if uri.starts_with("file:") || !uri.contains(':') {
            return WorkspacePackage::find_for(path).is_some();
        }
        false
    }

    /// Whether the library [library] is in this package (Dart
    /// `_isLibraryInWorkspacePackage`).
    pub fn contains_library(&self, ctx: &Ctx<'_>, library: EId<LibraryElement>) -> bool {
        let source = &ctx.fragment(ctx.get(library).first_fragment()).source;
        self.contains(&source.uri, &source.path)
    }

    /// Dart `PubPackage.sourceIsInPublicApi`.
    pub fn source_is_in_public_api(&self, path: &str) -> bool {
        let lib = self.root.join("lib");
        let path = Path::new(path);
        path.starts_with(&lib) && !path.starts_with(lib.join("src"))
    }

    /// Dart `WorkspacePackageImpl.isInTestDirectory`.
    pub fn is_in_test_directory(&self, path: &str) -> bool {
        let path = Path::new(path);
        ["test", "integration_test", "test_driver", "testing"]
            .iter()
            .any(|name| path.starts_with(self.root.join(name)))
    }
}

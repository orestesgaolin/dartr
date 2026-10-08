// Dart source: pkg/analyzer/lib/src/summary2/reference_resolver.dart,
// pkg/analyzer/lib/src/summary2/named_type_builder.dart,
// pkg/analyzer/lib/src/summary2/function_type_builder.dart,
// pkg/analyzer/lib/src/summary2/record_type_builder.dart,
// pkg/analyzer/lib/src/summary2/type_builder.dart,
// pkg/analyzer/lib/src/summary2/default_types_builder.dart,
// pkg/analyzer/lib/src/summary2/types_builder.dart,
// pkg/analyzer/lib/src/dart/resolver/scope_context.dart (the parts that
// type resolution uses)

//! Type resolution of linking (units B2 and B3, `Linker._resolveTypes`).
//!
//! The Dart code resolves each type annotation to a `TypeBuilder` (a
//! `TypeImpl` that is built later) and stores it in `node.type`. Builders
//! can be nested (`List<Foo>`) and are also stored in elements before they
//! are built (bounds, defaults). Interned types cannot hold builders, so
//! this port keeps a linking type [`LType`] (a built [`TypeId`] or a
//! builder index) in side tables: the type of each annotation node, the
//! pending bounds and defaults of the type parameters of the cycle.
//! Building a builder interns the type and caches it.

use std::sync::Arc;

use dartr_ast::NamedType as NamedTypeNode;
use dartr_ast::*;
use dartr_element::NamedType as RecordNamedField;
use dartr_element::*;
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_typesystem::TypeExt;
use dartr_typesystem::type_algebra::get_fresh_type_parameters;
use indexmap::{IndexMap, IndexSet};

use crate::ast_util::*;
use crate::link::Linker;
use crate::scope::{EnclosedScope, LibraryScopes, ScopeElement, ScopeLookupResult, lookup_name};

/// A node of a unit of the cycle: (library builder, unit, node).
pub type NodeKey = (u32, u32, NodeId);

/// A type of linking: built, or a builder (index in
/// [`TypeResolution::builders`]).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum LType {
    Built(TypeId),
    Builder(u32),
}

/// A formal parameter of a function type builder (Dart
/// `FormalParameterElementImpl.synthetic`).
#[derive(Clone, Debug)]
pub struct LParam {
    pub name: Option<Name>,
    pub ty: LType,
    pub kind: ParameterKind,
}

/// The Dart `TypeBuilder` subclasses.
#[derive(Clone, Debug)]
pub enum BuilderKind {
    /// `NamedTypeBuilder`; `element` is `None` for a name that is not a
    /// type element (a multiply defined name, a prefix).
    Named {
        element: Option<ElementId>,
        arguments: Vec<LType>,
        nullability: Nullability,
        node: Option<NodeKey>,
    },
    /// `FunctionTypeBuilder`.
    Function {
        type_params: Vec<EId<TypeParameterElement>>,
        params: Vec<LParam>,
        return_type: LType,
        nullability: Nullability,
        node: Option<NodeKey>,
    },
    /// `RecordTypeBuilder`.
    Record {
        node: NodeKey,
        positional: Vec<LType>,
        named: Vec<(Name, LType)>,
        nullability: Nullability,
    },
}

#[derive(Clone, Debug)]
pub struct Builder {
    pub kind: BuilderKind,
    cache: Option<TypeId>,
    building: bool,
}

/// Dart `Variance.combine`.
pub fn variance_combine(a: Variance, b: Variance) -> Variance {
    use Variance::*;
    if a == Unrelated || b == Unrelated {
        return Unrelated;
    }
    if a == Invariant || b == Invariant {
        return Invariant;
    }
    if a == b { Covariant } else { Contravariant }
}

/// Dart `Variance.meet` (the bitwise or of the encodings).
pub fn variance_meet(a: Variance, b: Variance) -> Variance {
    use Variance::*;
    let code = |v: Variance| match v {
        Unrelated => 0,
        Covariant => 1,
        Contravariant => 2,
        Invariant => 3,
    };
    match code(a) | code(b) {
        0 => Unrelated,
        1 => Covariant,
        2 => Contravariant,
        _ => Invariant,
    }
}

/// Dart `TypeParameterElementImpl.variance` (covariant when not declared).
pub fn type_parameter_variance(ctx: &Ctx<'_>, p: EId<TypeParameterElement>) -> Variance {
    ctx.get(p).variance.unwrap_or(Variance::Covariant)
}

/// The state of type resolution of one cycle.
#[derive(Default)]
pub struct TypeResolution {
    pub builders: Vec<Builder>,
    /// Dart `TypeAnnotation.type`.
    pub node_types: IndexMap<NodeKey, LType>,
    /// Dart `NamedType.element`.
    pub node_elements: IndexMap<NodeKey, Option<ScopeElement>>,
    /// Dart `NodesToBuildType.declarations`.
    pub declarations: Vec<NodeKey>,
    /// Dart `NodesToBuildType.typeBuilders`.
    pub type_builders: Vec<u32>,
    /// `TypeParameterElementImpl.bound` set before it is built.
    pub pending_bounds: IndexMap<EId<TypeParameterElement>, LType>,
    /// `TypeParameterElementImpl.defaultType` set before it is built.
    pub pending_defaults: IndexMap<EId<TypeParameterElement>, LType>,
    /// The `boundNode.type = dynamic` of `_breakSelfCycles` and
    /// `_breakRawTypeCycles`: Dart overwrites `node.type`, and the builder
    /// sets it again when it is built, so it affects only the default
    /// types.
    pub bound_overrides: IndexMap<NodeKey, LType>,
}

/// The AST of a unit of the cycle.
pub fn unit_ast<'l>(lk: &'l Linker<'_>, lib: u32, unit: u32) -> &'l Ast {
    &lk.builders[lib as usize].units[unit as usize].parsed.ast
}

/// Dart `node.declaredFragment`.
pub fn declared_fragment(lk: &Linker<'_>, key: NodeKey) -> Option<FragmentId> {
    lk.builders[key.0 as usize].units[key.1 as usize]
        .declared_fragments
        .get(&key.2)
        .copied()
}

/// Dart `node.declaredFragment!.element`.
pub fn declared_element(lk: &Linker<'_>, key: NodeKey) -> Option<ElementId> {
    let f = declared_fragment(lk, key)?;
    lk.core.store.fragment_data(f)?.element.try_get().copied()
}

impl TypeResolution {
    fn add_builder(&mut self, kind: BuilderKind) -> LType {
        self.builders.push(Builder {
            kind,
            cache: None,
            building: false,
        });
        LType::Builder(self.builders.len() as u32 - 1)
    }

    /// Dart `node.type` (built when the builder is built).
    pub fn node_type(&self, key: NodeKey) -> Option<LType> {
        let t = *self.node_types.get(&key)?;
        Some(match t {
            LType::Builder(b) => match self.builders[b as usize].cache {
                Some(built) => LType::Built(built),
                None => t,
            },
            t => t,
        })
    }

    /// `node.type` as `DefaultTypesBuilder` reads it.
    fn default_node_type(&self, key: NodeKey) -> Option<LType> {
        match self.bound_overrides.get(&key) {
            Some(&t) => Some(t),
            None => self.node_type(key),
        }
    }

    /// The bound of [p] (pending or set).
    pub fn bound_of(&self, ctx: &Ctx<'_>, p: EId<TypeParameterElement>) -> Option<LType> {
        if let Some(&b) = self.pending_bounds.get(&p) {
            return Some(b);
        }
        ctx.get(p).bound.get().map(LType::Built)
    }

    /// Dart `_buildType`: builds [t] if it is a builder.
    pub fn build_type(&mut self, lk: &Linker<'_>, ctx: &Ctx<'_>, t: LType) -> TypeId {
        match t {
            LType::Built(t) => t,
            LType::Builder(b) => self.build(lk, ctx, b),
        }
    }

    /// `TypeBuilder.build()`.
    pub fn build(&mut self, lk: &Linker<'_>, ctx: &Ctx<'_>, b: u32) -> TypeId {
        if let Some(t) = self.builders[b as usize].cache {
            return t;
        }
        let kind = self.builders[b as usize].kind.clone();
        let result = match kind {
            BuilderKind::Named {
                element,
                arguments,
                nullability,
                ..
            } => self.build_named(lk, ctx, element, &arguments, nullability),
            BuilderKind::Function {
                type_params,
                params,
                return_type,
                nullability,
                ..
            } => self.build_function(lk, ctx, &type_params, &params, return_type, nullability),
            BuilderKind::Record {
                positional,
                named,
                nullability,
                ..
            } => {
                if self.builders[b as usize].building {
                    // Dart `_buildRecordType(recursionFound: true)`.
                    let p = vec![TypeId::DYNAMIC; positional.len()];
                    let n: Vec<RecordNamedField> = named
                        .iter()
                        .map(|(name, _)| RecordNamedField {
                            name: *name,
                            ty: TypeId::DYNAMIC,
                        })
                        .collect();
                    let t = ctx.record_type(&p, &n, nullability, None);
                    self.builders[b as usize].cache = Some(t);
                    return t;
                }
                self.builders[b as usize].building = true;
                let p: Vec<TypeId> = positional.iter().map(|&t| self.build_type(lk, ctx, t)).collect();
                let n: Vec<RecordNamedField> = named
                    .iter()
                    .map(|&(name, t)| RecordNamedField {
                        name,
                        ty: self.build_type(lk, ctx, t),
                    })
                    .collect();
                self.builders[b as usize].building = false;
                if let Some(t) = self.builders[b as usize].cache {
                    return t;
                }
                ctx.record_type(&p, &n, nullability, None)
            }
        };
        self.builders[b as usize].cache = Some(result);
        result
    }

    /// `NamedTypeBuilder.build`.
    fn build_named(
        &mut self,
        lk: &Linker<'_>,
        ctx: &Ctx<'_>,
        element: Option<ElementId>,
        arguments: &[LType],
        nullability: Nullability,
    ) -> TypeId {
        let Some(element) = element else {
            return TypeId::INVALID;
        };
        if let Some(i) = element.cast::<InterfaceElement>() {
            let parameters = ctx.instance(i.upcast()).type_params.clone();
            let args = self.build_arguments(lk, ctx, &parameters, arguments);
            return ctx.interface_type(i, &args, nullability);
        }
        if let Some(a) = element.cast::<TypeAliasElement>() {
            let aliased = self.get_aliased_type(lk, ctx, a);
            let parameters = ctx.get(a).type_params.clone();
            let args = self.build_arguments(lk, ctx, &parameters, arguments);
            ctx.get(a).aliased_type.set(Some(aliased));
            return ctx.instantiate_type_alias(a, &args, nullability);
        }
        if element == ElementId::NEVER {
            return ctx.never_type(nullability);
        }
        if let Some(p) = element.cast::<TypeParameterElement>() {
            return ctx.type_parameter_type(p, nullability);
        }
        if element == ElementId::DYNAMIC {
            return TypeId::DYNAMIC;
        }
        TypeId::INVALID
    }

    /// `NamedTypeBuilder._buildArguments`.
    fn build_arguments(
        &mut self,
        lk: &Linker<'_>,
        ctx: &Ctx<'_>,
        parameters: &[EId<TypeParameterElement>],
        arguments: &[LType],
    ) -> Vec<TypeId> {
        if parameters.is_empty() {
            Vec::new()
        } else if !arguments.is_empty() {
            if arguments.len() == parameters.len() {
                arguments.iter().map(|&a| self.build_type(lk, ctx, a)).collect()
            } else {
                vec![TypeId::DYNAMIC; parameters.len()]
            }
        } else {
            parameters
                .iter()
                .map(|&p| {
                    let default = match self.pending_defaults.get(&p) {
                        Some(&d) => d,
                        None => LType::Built(ctx.get(p).default_type.get().unwrap_or(TypeId::DYNAMIC)),
                    };
                    self.build_type(lk, ctx, default)
                })
                .collect()
        }
    }

    /// `NamedTypeBuilder._getAliasedType`.
    fn get_aliased_type(&mut self, lk: &Linker<'_>, ctx: &Ctx<'_>, element: EId<TypeAliasElement>) -> TypeId {
        if element.store() != lk.core.store.id {
            return ctx.get(element).aliased_type.get().unwrap_or(TypeId::INVALID);
        }
        let first = ctx.get(element).first_fragment();
        let Some(&(lib, unit, node)) = lk.core.fragment_nodes.get(&first.raw()) else {
            return TypeId::DYNAMIC;
        };
        if let Some(existing) = ctx.get(element).aliased_type.get() {
            return existing;
        }
        ctx.get(element).aliased_type.set(Some(TypeId::DYNAMIC));
        let ast = unit_ast(lk, lib as u32, unit as u32);
        let key = |n: NodeId| (lib as u32, unit as u32, n);
        if let Some(f) = ast.cast::<FunctionTypeAlias>(node) {
            let f = ast.get(f);
            let result = self.build_function_type_from_nodes(
                lk,
                ctx,
                lib as u32,
                unit as u32,
                None,
                f.return_type,
                f.parameters,
                Nullability::None,
            );
            ctx.get(element).aliased_type.set(Some(result));
            return result;
        }
        if let Some(g) = ast.cast::<GenericTypeAlias>(node) {
            let type_node = ast.get(g).type_;
            // Dart `_buildAliasedType`: both branches build the node type
            // when the aliased type is a function type.
            let aliased = if lk.builders[lib].is_enabled(ExperimentalFlag::NonfunctionTypeAliases)
                || ast.is::<GenericFunctionType>(type_node.raw())
            {
                match self.node_type(key(type_node.raw())) {
                    Some(t) => self.build_type(lk, ctx, t),
                    None => TypeId::DYNAMIC,
                }
            } else {
                ctx.function_type(&[], &[], TypeId::DYNAMIC, Nullability::None, None)
            };
            ctx.get(element).aliased_type.set(Some(aliased));
            return aliased;
        }
        TypeId::DYNAMIC
    }

    /// The type of a type annotation node, `dynamic` when absent (Dart
    /// `_buildNodeType`).
    fn build_node_type(&mut self, lk: &Linker<'_>, ctx: &Ctx<'_>, lib: u32, unit: u32, node: Option<Id<TypeAnnotation>>) -> TypeId {
        match node.and_then(|n| self.node_type((lib, unit, n.raw()))) {
            Some(t) => self.build_type(lk, ctx, t),
            None => TypeId::DYNAMIC,
        }
    }

    /// `NamedTypeBuilder._buildFunctionType` (synthetic parameters built
    /// from the nodes).
    #[allow(clippy::too_many_arguments)]
    fn build_function_type_from_nodes(
        &mut self,
        lk: &Linker<'_>,
        ctx: &Ctx<'_>,
        lib: u32,
        unit: u32,
        type_parameters: Option<Id<TypeParameterList>>,
        return_type: Option<Id<TypeAnnotation>>,
        formal_parameters: Id<FormalParameterList>,
        nullability: Nullability,
    ) -> TypeId {
        let ret = self.build_node_type(lk, ctx, lib, unit, return_type);
        let ast = unit_ast(lk, lib, unit);
        let tps: Vec<EId<TypeParameterElement>> = match type_parameters {
            Some(list) => ast
                .list(ast.get(list).type_parameters)
                .iter()
                .filter_map(|tp| declared_element(lk, (lib, unit, tp.raw())).and_then(|e| e.cast()))
                .collect(),
            None => Vec::new(),
        };
        let mut params = Vec::new();
        for &p in ast.list(ast.get(formal_parameters).parameters) {
            let (name, _, suffix) = crate::informative_data::formal_parameter_parts(ast, p);
            let name_text = name.map(|t| ast.tokens.lexeme(t)).unwrap_or("");
            let kind = formal_parameter_kind(ast, p);
            let ty = match suffix {
                Some(s) => {
                    let s2 = ast.get(s);
                    let n = if s2.question.is_some() {
                        Nullability::Question
                    } else {
                        Nullability::None
                    };
                    self.build_function_type_from_nodes(
                        lk,
                        ctx,
                        lib,
                        unit,
                        s2.type_parameters,
                        formal_parameter_type_node(ast, p),
                        s2.formal_parameters,
                        n,
                    )
                }
                None => {
                    if ast.is::<RegularFormalParameter>(p.raw()) {
                        self.build_node_type(lk, ctx, lib, unit, formal_parameter_type_node(ast, p))
                    } else {
                        // Dart throws `UnimplementedError` for a field or super
                        // formal parameter in a function type.
                        TypeId::DYNAMIC
                    }
                }
            };
            params.push(FnParam {
                name: Some(lk.core.name(name_text)),
                kind,
                ty,
                covariant: false,
                element: None,
            });
        }
        ctx.function_type(&tps, &params, ret, nullability, None)
    }

    /// `FunctionTypeBuilder.build`.
    fn build_function(
        &mut self,
        lk: &Linker<'_>,
        ctx: &Ctx<'_>,
        type_params: &[EId<TypeParameterElement>],
        params: &[LParam],
        return_type: LType,
        nullability: Nullability,
    ) -> TypeId {
        for &tp in type_params {
            if let Some(bound) = self.bound_of(ctx, tp) {
                let built = self.build_type(lk, ctx, bound);
                self.pending_bounds.shift_remove(&tp);
                ctx.get(tp).bound.set(Some(built));
            }
        }
        let built_params: Vec<FnParam> = params
            .iter()
            .map(|p| FnParam {
                name: p.name,
                kind: p.kind,
                ty: self.build_type(lk, ctx, p.ty),
                covariant: false,
                element: None,
            })
            .collect();
        let ret = self.build_type(lk, ctx, return_type);
        let t = ctx.function_type(type_params, &built_params, ret, nullability, None);
        let fresh = get_fresh_type_parameters(ctx, type_params);
        fresh.apply_to_function_type(ctx, t)
    }

    /// `FunctionTypeBuilder.getParameters`.
    fn function_builder_parameters(&mut self, lk: &Linker<'_>, lib: u32, unit: u32, list: Id<FormalParameterList>) -> Vec<LParam> {
        let ast = unit_ast(lk, lib, unit);
        let mut out = Vec::new();
        for &p in ast.list(ast.get(list).parameters) {
            let (name, _, suffix) = crate::informative_data::formal_parameter_parts(ast, p);
            let name = name
                .map(|t| ast.tokens.lexeme(t))
                .filter(|n| !n.is_empty())
                .map(|n| lk.core.name(n));
            let ty = match suffix {
                Some(s) => {
                    let s2 = ast.get(s);
                    let nullability = if s2.question.is_some() {
                        Nullability::Question
                    } else {
                        Nullability::None
                    };
                    let type_params = type_parameter_elements(lk, lib, unit, s2.type_parameters);
                    let params = self.function_builder_parameters(lk, lib, unit, s2.formal_parameters);
                    let return_type = self.node_type_or_dynamic(lib, unit, formal_parameter_type_node(ast, p));
                    self.add_builder(BuilderKind::Function {
                        type_params,
                        params,
                        return_type,
                        nullability,
                        node: None,
                    })
                }
                None => self.node_type_or_dynamic(lib, unit, formal_parameter_type_node(ast, p)),
            };
            out.push(LParam {
                name,
                ty,
                kind: formal_parameter_kind(ast, p),
            });
        }
        out
    }

    /// Dart `_getNodeType`.
    fn node_type_or_dynamic(&self, lib: u32, unit: u32, node: Option<Id<TypeAnnotation>>) -> LType {
        node.and_then(|n| self.node_type((lib, unit, n.raw())))
            .unwrap_or(LType::Built(TypeId::DYNAMIC))
    }
}

/// The elements of the type parameters of a type parameter list node.
fn type_parameter_elements(lk: &Linker<'_>, lib: u32, unit: u32, list: Option<Id<TypeParameterList>>) -> Vec<EId<TypeParameterElement>> {
    let Some(list) = list else { return Vec::new() };
    let ast = unit_ast(lk, lib, unit);
    ast.list(ast.get(list).type_parameters)
        .iter()
        .filter_map(|tp| declared_element(lk, (lib, unit, tp.raw())).and_then(|e| e.cast()))
        .collect()
}

/// Dart `FormalParameter.kind`.
pub fn formal_parameter_kind(ast: &Ast, p: Id<FormalParameter>) -> ParameterKind {
    let p = p.raw();
    if let Some(r) = ast.cast::<RegularFormalParameter>(p) {
        ast.get(r).kind
    } else if let Some(f) = ast.cast::<FieldFormalParameter>(p) {
        ast.get(f).kind
    } else {
        ast.get(ast.cast::<SuperFormalParameter>(p).unwrap()).kind
    }
}

/// Dart `FormalParameter.type`.
pub fn formal_parameter_type_node(ast: &Ast, p: Id<FormalParameter>) -> Option<Id<TypeAnnotation>> {
    let p = p.raw();
    if let Some(r) = ast.cast::<RegularFormalParameter>(p) {
        ast.get(r).type_
    } else if let Some(f) = ast.cast::<FieldFormalParameter>(p) {
        ast.get(f).type_
    } else {
        ast.get(ast.cast::<SuperFormalParameter>(p).unwrap()).type_
    }
}

// ---- ReferenceResolver ----

/// Dart `ReferenceResolver` with its `ScopeContext`.
pub struct ReferenceResolver<'r, 'l, 'a> {
    tr: &'r mut TypeResolution,
    lk: &'l Linker<'a>,
    ctx: &'r Ctx<'l>,
    scopes: &'r LibraryScopes,
    lib: u32,
    unit: u32,
    fragment: FId<LibraryFragment>,
    frames: Vec<EnclosedScope>,
    in_static_member: bool,
    wildcard_variables: bool,
}

impl<'r, 'l, 'a> ReferenceResolver<'r, 'l, 'a> {
    pub fn new(
        tr: &'r mut TypeResolution,
        lk: &'l Linker<'a>,
        ctx: &'r Ctx<'l>,
        scopes: &'r LibraryScopes,
        lib: u32,
        unit: u32,
    ) -> Self {
        let builder = &lk.builders[lib as usize];
        ReferenceResolver {
            tr,
            lk,
            ctx,
            scopes,
            lib,
            unit,
            fragment: builder.units[unit as usize].fragment,
            frames: Vec::new(),
            in_static_member: false,
            wildcard_variables: builder.is_enabled(ExperimentalFlag::WildcardVariables),
        }
    }

    fn ast(&self) -> &'l Ast {
        unit_ast(self.lk, self.lib, self.unit)
    }

    fn key(&self, n: impl Into<NodeId>) -> NodeKey {
        (self.lib, self.unit, n.into())
    }

    /// `nameScope.lookup(id)`.
    fn lookup(&self, id: &str) -> ScopeLookupResult {
        for frame in self.frames.iter().rev() {
            if let Some(r) = frame.lookup(id) {
                return r;
            }
        }
        self.scopes.lookup(self.fragment, id)
    }

    fn with_frame(&mut self, frame: EnclosedScope, f: impl FnOnce(&mut Self)) {
        self.frames.push(frame);
        f(self);
        self.frames.pop();
    }

    /// `withTypeParameterScope(elements)`.
    fn with_type_parameter_scope(&mut self, elements: &[EId<TypeParameterElement>], f: impl FnOnce(&mut Self)) {
        let mut map = IndexMap::new();
        for &e in elements {
            let Some(name) = self.ctx.get(e).name else { continue };
            let name = self.lk.core.name_str(name);
            if self.wildcard_variables && name == "_" {
                continue;
            }
            map.entry(Arc::from(name)).or_insert(e.raw());
        }
        self.with_frame(EnclosedScope::TypeParameters(map), f);
    }

    /// `withTypeParameterList(node)`.
    fn with_type_parameter_list(&mut self, list: Option<Id<TypeParameterList>>, f: impl FnOnce(&mut Self)) {
        let elements = type_parameter_elements(self.lk, self.lib, self.unit, list);
        self.with_type_parameter_scope(&elements, f);
    }

    /// `withInstanceScope(element)` / `withExtensionScope(element)`.
    fn with_instance_scope(&mut self, element: ElementId, f: impl FnOnce(&mut Self)) {
        let ctx = self.ctx;
        let i = ctx.instance(EId::from_raw(element));
        let is_static = |e: ElementId| -> bool {
            let first = ctx.element_data(e).unwrap().first_fragment;
            ctx.fragment_data(first)
                .unwrap()
                .flags
                .has(FragmentFlags::EXECUTABLE_FRAGMENT_IS_STATIC)
        };
        let mut getters: IndexMap<Arc<str>, (ElementId, bool)> = IndexMap::new();
        let mut setters: IndexMap<Arc<str>, (ElementId, bool)> = IndexMap::new();
        for e in i.getters.iter().map(|e| e.raw()).chain(i.methods.iter().map(|e| e.raw())) {
            if let Some(n) = lookup_name(self.lk, e) {
                getters.entry(n.into()).or_insert((e, is_static(e)));
            }
        }
        for e in i.setters.iter().map(|e| e.raw()) {
            if let Some(n) = lookup_name(self.lk, e)
                && let Some(id) = n.strip_suffix('=')
            {
                setters.entry(id.into()).or_insert((e, is_static(e)));
            }
        }
        let frame = if element.tag() == Tag::Extension {
            EnclosedScope::Extension {
                getters: getters.into_iter().map(|(k, v)| (k, v.0)).collect(),
                setters: setters.into_iter().map(|(k, v)| (k, v.0)).collect(),
            }
        } else {
            EnclosedScope::Instance { getters, setters }
        };
        self.with_frame(frame, f);
    }

    fn with_in_static_member(&mut self, is_static: bool, f: impl FnOnce(&mut Self)) {
        let saved = self.in_static_member;
        self.in_static_member = is_static;
        f(self);
        self.in_static_member = saved;
    }

    fn element_of(&self, node: impl Into<NodeId>) -> Option<ElementId> {
        declared_element(self.lk, self.key(node))
    }

    fn type_params_of(&self, element: Option<ElementId>) -> Vec<EId<TypeParameterElement>> {
        let Some(e) = element else { return Vec::new() };
        match e.tag() {
            Tag::Class | Tag::Enum | Tag::Mixin | Tag::Extension | Tag::ExtensionType => {
                self.ctx.instance(EId::from_raw(e)).type_params.clone()
            }
            Tag::TypeAlias => self.ctx.get(EId::<TypeAliasElement>::from_raw(e)).type_params.clone(),
            Tag::Method | Tag::Constructor | Tag::Getter | Tag::Setter | Tag::TopLevelFunction => {
                self.ctx.executable(EId::from_raw(e)).type_params.clone()
            }
            _ => Vec::new(),
        }
    }

    /// Dart `visitCompilationUnit`.
    pub fn resolve_unit(&mut self) {
        let ast = self.ast();
        let unit = self.lk.builders[self.lib as usize].units[self.unit as usize].parsed.unit;
        for &d in ast.list(ast.get(unit).declarations) {
            self.visit_declaration(d.raw());
        }
    }

    fn visit_declaration(&mut self, d: NodeId) {
        let ast = self.ast();
        if let Some(n) = ast.cast::<ClassDeclaration>(d) {
            let n = ast.get(n);
            let element = self.element_of(d);
            let tps = self.type_params_of(element);
            self.with_type_parameter_scope(&tps, |r| {
                if let Some(tpl) = class_name_part_type_parameters(ast, n.name_part) {
                    r.visit_type_parameter_list(tpl);
                }
                if let Some(e) = n.extends_clause {
                    r.visit_type(ast.get(e).superclass.upcast());
                }
                if let Some(w) = n.with_clause {
                    r.visit_named_types(ast.get(w).mixin_types);
                }
                if let Some(i) = n.implements_clause {
                    r.visit_named_types(ast.get(i).interfaces);
                }
                if let Some(element) = element {
                    r.with_instance_scope(element, |r| {
                        if let Some(p) = ast.cast::<PrimaryConstructorDeclaration>(n.name_part.raw()) {
                            r.visit_formal_parameter_list(ast.get(p).formal_parameters);
                        }
                        for m in class_body_members(ast, n.body) {
                            r.visit_class_member(m.raw());
                        }
                    });
                }
            });
            self.tr.declarations.push(self.key(d));
        } else if let Some(n) = ast.cast::<ClassTypeAlias>(d) {
            let n = ast.get(n);
            let tps = self.type_params_of(self.element_of(d));
            self.with_type_parameter_scope(&tps, |r| {
                if let Some(tpl) = n.type_parameters {
                    r.visit_type_parameter_list(tpl);
                }
                r.visit_type(n.superclass.upcast());
                r.visit_named_types(ast.get(n.with_clause).mixin_types);
                if let Some(i) = n.implements_clause {
                    r.visit_named_types(ast.get(i).interfaces);
                }
            });
            self.tr.declarations.push(self.key(d));
        } else if let Some(n) = ast.cast::<EnumDeclaration>(d) {
            let n = ast.get(n);
            let element = self.element_of(d);
            let tps = self.type_params_of(element);
            self.with_type_parameter_scope(&tps, |r| {
                if let Some(tpl) = class_name_part_type_parameters(ast, n.name_part) {
                    r.visit_type_parameter_list(tpl);
                }
                if let Some(w) = n.with_clause {
                    r.visit_named_types(ast.get(w).mixin_types);
                }
                if let Some(i) = n.implements_clause {
                    r.visit_named_types(ast.get(i).interfaces);
                }
                if let Some(element) = element {
                    r.with_instance_scope(element, |r| {
                        if let Some(p) = ast.cast::<PrimaryConstructorDeclaration>(n.name_part.raw()) {
                            r.visit_formal_parameter_list(ast.get(p).formal_parameters);
                        }
                        for m in enum_body_members(ast, n.body) {
                            r.visit_class_member(m.raw());
                        }
                    });
                }
            });
            self.tr.declarations.push(self.key(d));
        } else if let Some(n) = ast.cast::<ExtensionDeclaration>(d) {
            let n = ast.get(n);
            let element = self.element_of(d);
            let tps = self.type_params_of(element);
            self.with_type_parameter_scope(&tps, |r| {
                if let Some(tpl) = n.type_parameters {
                    r.visit_type_parameter_list(tpl);
                }
                if let Some(on) = n.on_clause {
                    r.visit_type(ast.get(on).extended_type);
                }
                if let Some(element) = element {
                    r.with_instance_scope(element, |r| {
                        for m in class_body_members(ast, n.body) {
                            r.visit_class_member(m.raw());
                        }
                    });
                }
            });
            self.tr.declarations.push(self.key(d));
        } else if let Some(n) = ast.cast::<ExtensionTypeDeclaration>(d) {
            let n = ast.get(n);
            let element = self.element_of(d);
            let tps = self.type_params_of(element);
            let primary = self.lk.builders[self.lib as usize].is_enabled(ExperimentalFlag::PrimaryConstructors);
            self.with_type_parameter_scope(&tps, |r| {
                if let Some(tpl) = class_name_part_type_parameters(ast, n.name_part) {
                    r.visit_type_parameter_list(tpl);
                }
                if let Some(i) = n.implements_clause {
                    r.visit_named_types(ast.get(i).interfaces);
                }
                let primary_params = ast
                    .cast::<PrimaryConstructorDeclaration>(n.name_part.raw())
                    .map(|p| ast.get(p).formal_parameters);
                if !primary && let Some(p) = primary_params {
                    r.visit_formal_parameter_list(p);
                }
                if let Some(element) = element {
                    r.with_instance_scope(element, |r| {
                        if primary && let Some(p) = primary_params {
                            r.visit_formal_parameter_list(p);
                        }
                        for m in class_body_members(ast, n.body) {
                            r.visit_class_member(m.raw());
                        }
                    });
                }
            });
            self.tr.declarations.push(self.key(d));
        } else if let Some(n) = ast.cast::<FunctionDeclaration>(d) {
            let n = ast.get(n);
            let tps = self.type_params_of(self.element_of(d));
            let fe = ast.get(n.function_expression);
            self.with_type_parameter_scope(&tps, |r| {
                if let Some(t) = n.return_type {
                    r.visit_type(t);
                }
                if let Some(tpl) = fe.type_parameters {
                    r.visit_type_parameter_list(tpl);
                }
                if let Some(p) = fe.parameters {
                    r.visit_formal_parameter_list(p);
                }
            });
            self.tr.declarations.push(self.key(d));
        } else if let Some(n) = ast.cast::<FunctionTypeAlias>(d) {
            let n = ast.get(n);
            let tps = self.type_params_of(self.element_of(d));
            self.with_type_parameter_scope(&tps, |r| {
                if let Some(t) = n.return_type {
                    r.visit_type(t);
                }
                if let Some(tpl) = n.type_parameters {
                    r.visit_type_parameter_list(tpl);
                }
                r.visit_formal_parameter_list(n.parameters);
            });
            self.tr.declarations.push(self.key(d));
        } else if let Some(n) = ast.cast::<GenericTypeAlias>(d) {
            let n = ast.get(n);
            let tps = self.type_params_of(self.element_of(d));
            self.with_type_parameter_scope(&tps, |r| {
                if let Some(tpl) = n.type_parameters {
                    r.visit_type_parameter_list(tpl);
                }
                r.visit_type(n.type_);
            });
            self.tr.declarations.push(self.key(d));
        } else if let Some(n) = ast.cast::<MixinDeclaration>(d) {
            let n = ast.get(n);
            let element = self.element_of(d);
            let tps = self.type_params_of(element);
            self.with_type_parameter_scope(&tps, |r| {
                if let Some(tpl) = n.type_parameters {
                    r.visit_type_parameter_list(tpl);
                }
                if let Some(on) = n.on_clause {
                    r.visit_named_types(ast.get(on).superclass_constraints);
                }
                if let Some(i) = n.implements_clause {
                    r.visit_named_types(ast.get(i).interfaces);
                }
                if let Some(element) = element {
                    r.with_instance_scope(element, |r| {
                        for m in class_body_members(ast, n.body) {
                            r.visit_class_member(m.raw());
                        }
                    });
                }
            });
            self.tr.declarations.push(self.key(d));
        } else if let Some(n) = ast.cast::<TopLevelVariableDeclaration>(d) {
            self.visit_variable_declaration_list(ast.get(n).variables);
        }
    }

    fn visit_class_member(&mut self, m: NodeId) {
        let ast = self.ast();
        if let Some(n) = ast.cast::<ConstructorDeclaration>(m) {
            // Dart `visitConstructorDeclaration`: the formal parameters.
            self.visit_formal_parameter_list(ast.get(n).parameters);
        } else if let Some(n) = ast.cast::<FieldDeclaration>(m) {
            let is_static = field_is_static(ast, n);
            let list = ast.get(n).fields;
            self.with_in_static_member(is_static, |r| r.visit_variable_declaration_list(list));
        } else if let Some(n) = ast.cast::<MethodDeclaration>(m) {
            let is_static = method_is_static(ast, n);
            let tps = self.type_params_of(self.element_of(m));
            let n2 = ast.get(n);
            self.with_in_static_member(is_static, |r| {
                r.with_type_parameter_scope(&tps, |r| {
                    if let Some(t) = n2.return_type {
                        r.visit_type(t);
                    }
                    if let Some(tpl) = n2.type_parameters {
                        r.visit_type_parameter_list(tpl);
                    }
                    if let Some(p) = n2.parameters {
                        r.visit_formal_parameter_list(p);
                    }
                });
            });
            self.tr.declarations.push(self.key(m));
        }
    }

    /// Dart `visitVariableDeclarationList`.
    fn visit_variable_declaration_list(&mut self, list: Id<VariableDeclarationList>) {
        let ast = self.ast();
        if let Some(t) = ast.get(list).type_ {
            self.visit_type(t);
        }
        self.tr.declarations.push(self.key(list));
    }

    fn visit_named_types(&mut self, list: NodeList<NamedTypeNode>) {
        let ast = self.ast();
        for &t in ast.list(list) {
            self.visit_type(t.upcast());
        }
    }

    fn visit_type_parameter_list(&mut self, list: Id<TypeParameterList>) {
        let ast = self.ast();
        for &tp in ast.list(ast.get(list).type_parameters) {
            self.visit_type_parameter(tp);
        }
    }

    /// Dart `visitTypeParameter`.
    fn visit_type_parameter(&mut self, node: Id<TypeParameter>) {
        let ast = self.ast();
        let Some(bound) = ast.get(node).bound else {
            return;
        };
        self.visit_type(bound);
        if let Some(f) = declared_fragment(self.lk, self.key(node))
            && self.ctx.fragment_data(f).unwrap().previous_fragment.is_none()
            && let Some(e) = self.element_of(node).and_then(|e| e.cast::<TypeParameterElement>())
            && let Some(t) = self.tr.node_types.get(&self.key(bound.raw())).copied()
        {
            self.tr.pending_bounds.insert(e, t);
        }
        self.tr.declarations.push(self.key(node));
    }

    fn visit_formal_parameter_list(&mut self, list: Id<FormalParameterList>) {
        let ast = self.ast();
        for &p in ast.list(ast.get(list).parameters) {
            self.visit_formal_parameter(p);
        }
    }

    /// Dart `_visitFormalParameter` + `ScopeContext.visitFormalParameter`.
    fn visit_formal_parameter(&mut self, p: Id<FormalParameter>) {
        let ast = self.ast();
        let (_, _, suffix) = crate::informative_data::formal_parameter_parts(ast, p);
        let type_node = formal_parameter_type_node(ast, p);
        match suffix {
            None => {
                if let Some(t) = type_node {
                    self.visit_type(t);
                }
            }
            Some(s) => {
                let s = ast.get(s);
                self.with_type_parameter_list(s.type_parameters, |r| {
                    if let Some(t) = type_node {
                        r.visit_type(t);
                    }
                    if let Some(tpl) = s.type_parameters {
                        r.visit_type_parameter_list(tpl);
                    }
                    r.visit_formal_parameter_list(s.formal_parameters);
                });
            }
        }
        self.tr.declarations.push(self.key(p));
    }

    /// Type annotations.
    fn visit_type(&mut self, t: Id<TypeAnnotation>) {
        let ast = self.ast();
        let raw = t.raw();
        if let Some(n) = ast.cast::<NamedTypeNode>(raw) {
            self.visit_named_type(n);
        } else if let Some(g) = ast.cast::<GenericFunctionType>(raw) {
            self.visit_generic_function_type(g);
        } else if let Some(r) = ast.cast::<RecordTypeAnnotation>(raw) {
            self.visit_record_type_annotation(r);
        }
    }

    /// Dart `visitNamedType`.
    fn visit_named_type(&mut self, node: Id<NamedTypeNode>) {
        let ast = self.ast();
        let n = ast.get(node);
        let key = self.key(node);
        let element: Option<ScopeElement>;
        if let Some(prefix) = n.import_prefix {
            let prefix_name = ast.tokens.lexeme(ast.get(prefix).name);
            let prefix_element = self.lookup(prefix_name).getter;
            element = match prefix_element {
                Some(ScopeElement::Prefix(_, scope)) => {
                    self.scopes.prefix_lookup(scope, ast.tokens.lexeme(n.name)).getter
                }
                _ => None,
            };
        } else {
            let name = ast.tokens.lexeme(n.name);
            if name == "void" {
                self.tr.node_types.insert(key, LType::Built(TypeId::VOID));
                return;
            }
            element = self.lookup(name).getter;
        }
        self.tr.node_elements.insert(key, element.clone());
        if let Some(args) = n.type_arguments {
            for &a in ast.list(ast.get(args).arguments) {
                self.visit_type(a);
            }
        }
        let nullability = if n.question.is_some() {
            Nullability::Question
        } else {
            Nullability::None
        };
        let t = match element {
            None => LType::Built(TypeId::INVALID),
            Some(ScopeElement::Element(e)) if e.tag() == Tag::TypeParameter => {
                let p = EId::<TypeParameterElement>::from_raw(e);
                // Dart `instantiateTypeParameter`.
                let enclosing_is_instance = self
                    .ctx
                    .element_data(e)
                    .and_then(|d| d.enclosing)
                    .is_some_and(|en| {
                        matches!(
                            en.tag(),
                            Tag::Class | Tag::Enum | Tag::Mixin | Tag::Extension | Tag::ExtensionType
                        )
                    });
                if self.in_static_member && enclosing_is_instance {
                    LType::Built(TypeId::INVALID)
                } else {
                    LType::Built(self.ctx.type_parameter_type(p, nullability))
                }
            }
            Some(e) => {
                let arguments: Vec<LType> = match n.type_arguments {
                    Some(args) => ast
                        .list(ast.get(args).arguments)
                        .iter()
                        .map(|a| {
                            self.tr
                                .node_types
                                .get(&self.key(a.raw()))
                                .copied()
                                .unwrap_or(LType::Built(TypeId::INVALID))
                        })
                        .collect(),
                    None => Vec::new(),
                };
                let b = self.tr.add_builder(BuilderKind::Named {
                    element: e.element(),
                    arguments,
                    nullability,
                    node: Some(key),
                });
                if let LType::Builder(id) = b {
                    self.tr.type_builders.push(id);
                }
                b
            }
        };
        self.tr.node_types.insert(key, t);
    }

    /// Dart `visitGenericFunctionType`.
    fn visit_generic_function_type(&mut self, node: Id<GenericFunctionType>) {
        let ast = self.ast();
        let n = ast.get(node);
        self.with_type_parameter_list(n.type_parameters, |r| {
            if let Some(tpl) = n.type_parameters {
                r.visit_type_parameter_list(tpl);
            }
            r.visit_formal_parameter_list(n.parameters);
            if let Some(t) = n.return_type {
                r.visit_type(t);
            }
        });
        let nullability = if n.question.is_some() {
            Nullability::Question
        } else {
            Nullability::None
        };
        let type_params = type_parameter_elements(self.lk, self.lib, self.unit, n.type_parameters);
        let params = self.tr.function_builder_parameters(self.lk, self.lib, self.unit, n.parameters);
        let return_type = self.tr.node_type_or_dynamic(self.lib, self.unit, n.return_type);
        let key = self.key(node);
        let b = self.tr.add_builder(BuilderKind::Function {
            type_params,
            params,
            return_type,
            nullability,
            node: Some(key),
        });
        self.tr.node_types.insert(key, b);
        self.tr.declarations.push(key);
        if let LType::Builder(id) = b {
            self.tr.type_builders.push(id);
        }
    }

    /// Dart `visitRecordTypeAnnotation`.
    fn visit_record_type_annotation(&mut self, node: Id<RecordTypeAnnotation>) {
        let ast = self.ast();
        let n = ast.get(node);
        let mut positional = Vec::new();
        for &f in ast.list(n.positional_fields) {
            let t = ast.get(f).type_;
            self.visit_type(t);
            positional.push(self.tr.node_type_or_dynamic(self.lib, self.unit, Some(t)));
        }
        let mut named = Vec::new();
        if let Some(nf) = n.named_fields {
            for &f in ast.list(ast.get(nf).fields) {
                let f = ast.get(f);
                self.visit_type(f.type_);
                named.push((
                    self.lk.core.name(ast.tokens.lexeme(f.name)),
                    self.tr.node_type_or_dynamic(self.lib, self.unit, Some(f.type_)),
                ));
            }
        }
        let nullability = if n.question.is_some() {
            Nullability::Question
        } else {
            Nullability::None
        };
        let key = self.key(node);
        let b = self.tr.add_builder(BuilderKind::Record {
            node: key,
            positional,
            named,
            nullability,
        });
        self.tr.node_types.insert(key, b);
        if let LType::Builder(id) = b {
            self.tr.type_builders.push(id);
        }
    }
}

// ---- DefaultTypesBuilder ----

impl TypeResolution {
    /// The `(fragment, type parameter list)` of a declaration node (Dart
    /// `_fragmentTypeParameters`).
    fn fragment_type_parameters(&self, lk: &Linker<'_>, key: NodeKey) -> Option<(FragmentId, Option<Id<TypeParameterList>>)> {
        let ast = unit_ast(lk, key.0, key.1);
        let n = key.2;
        let list = if let Some(c) = ast.cast::<ClassDeclaration>(n) {
            class_name_part_type_parameters(ast, ast.get(c).name_part)
        } else if let Some(c) = ast.cast::<ClassTypeAlias>(n) {
            ast.get(c).type_parameters
        } else if let Some(e) = ast.cast::<EnumDeclaration>(n) {
            class_name_part_type_parameters(ast, ast.get(e).name_part)
        } else if let Some(e) = ast.cast::<ExtensionDeclaration>(n) {
            ast.get(e).type_parameters
        } else if let Some(e) = ast.cast::<ExtensionTypeDeclaration>(n) {
            class_name_part_type_parameters(ast, ast.get(e).name_part)
        } else if let Some(f) = ast.cast::<FunctionTypeAlias>(n) {
            ast.get(f).type_parameters
        } else if let Some(g) = ast.cast::<GenericTypeAlias>(n) {
            ast.get(g).type_parameters
        } else if let Some(m) = ast.cast::<MixinDeclaration>(n) {
            ast.get(m).type_parameters
        } else if let Some(m) = ast.cast::<MethodDeclaration>(n) {
            ast.get(m).type_parameters
        } else {
            let f = ast.cast::<FunctionDeclaration>(n)?;
            ast.get(ast.get(f).function_expression).type_parameters
        };
        let fragment = declared_fragment(lk, key)?;
        Some((fragment, list))
    }

    /// `DefaultTypesBuilder.build`.
    pub fn build_default_types(&mut self, lk: &Linker<'_>, ctx: &Ctx<'_>) {
        let declarations = self.declarations.clone();
        // _computeDefaultTypes
        for &key in &declarations {
            let Some((fragment, Some(list))) = self.fragment_type_parameters(lk, key) else {
                continue;
            };
            if ctx.fragment_data(fragment).unwrap().previous_fragment.is_some() {
                continue;
            }
            let Some(&element) = ctx.fragment_data(fragment).unwrap().element.try_get() else {
                continue;
            };
            self.break_self_cycles(lk, key.0, key.1, list);
            self.break_raw_type_cycles(lk, ctx, element, key.0, key.1, list);
            // Dart `_TypeParametersGraph` finds the type parameters of a
            // bound by identity (`Map.identity()`); for a top-level function
            // the type parameter types in the bounds have other element
            // objects than `node.declaredFragment!.element` (but `==` ones,
            // so the replacement still applies), so its graph has no edges.
            // Observed with the oracle (`tests/language/inference_using_bounds/
            // f_bounded_mutually_recursive_test.dart`).
            let graph_edges = !unit_ast(lk, key.0, key.1).is::<FunctionDeclaration>(key.2);
            self.compute_default_type(lk, ctx, key.0, key.1, list, graph_edges);
        }
        // _buildDefaultTypes
        for &key in &declarations {
            let Some((_, Some(list))) = self.fragment_type_parameters(lk, key) else {
                continue;
            };
            let ast = unit_ast(lk, key.0, key.1);
            for &tp in ast.list(ast.get(list).type_parameters) {
                let Some(e) = declared_element(lk, (key.0, key.1, tp.raw())).and_then(|e| e.cast::<TypeParameterElement>()) else {
                    continue;
                };
                if let Some(&d) = self.pending_defaults.get(&e) {
                    let built = self.build_type(lk, ctx, d);
                    self.pending_defaults.shift_remove(&e);
                    ctx.get(e).default_type.set(Some(built));
                }
            }
        }
    }

    /// `_breakSelfCycles`.
    fn break_self_cycles(&mut self, lk: &Linker<'_>, lib: u32, unit: u32, list: Id<TypeParameterList>) {
        let ast = unit_ast(lk, lib, unit);
        let type_parameters = ast.list(ast.get(list).type_parameters);
        let mut by_name: Option<IndexMap<&str, Id<TypeParameter>>> = None;
        for &parameter in type_parameters {
            let Some(bound) = ast.get(parameter).bound else { continue };
            if !ast.is::<NamedTypeNode>(bound.raw()) {
                continue;
            }
            let by_name = by_name.get_or_insert_with(|| {
                let mut m = IndexMap::new();
                for &p in type_parameters {
                    m.insert(ast.tokens.lexeme(ast.get(p).name), p);
                }
                m
            });
            let mut current = Some(parameter);
            let mut step = 0;
            while let Some(c) = current {
                if step >= type_parameters.len() {
                    break;
                }
                step += 1;
                let next = ast.get(c).bound.and_then(|b| ast.cast::<NamedTypeNode>(b.raw())).and_then(|b| {
                    let b = ast.get(b);
                    if b.import_prefix.is_none() {
                        by_name.get(ast.tokens.lexeme(b.name)).copied()
                    } else {
                        None
                    }
                });
                match next {
                    Some(n) => current = Some(n),
                    None => {
                        current = None;
                        break;
                    }
                }
            }
            if current.is_some() {
                self.bound_overrides.insert((lib, unit, bound.raw()), LType::Built(TypeId::DYNAMIC));
            }
        }
    }

    /// `_breakRawTypeCycles`.
    fn break_raw_type_cycles(
        &mut self,
        lk: &Linker<'_>,
        ctx: &Ctx<'_>,
        declaration: ElementId,
        lib: u32,
        unit: u32,
        list: Id<TypeParameterList>,
    ) {
        let ast = unit_ast(lk, lib, unit);
        let mut all_cycles: Vec<Vec<NodeKey>> = Vec::new();
        for &parameter in ast.list(ast.get(list).type_parameters) {
            let Some(bound) = ast.get(parameter).bound else { continue };
            let Some(t) = self.default_node_type((lib, unit, bound.raw())) else { continue };
            let mut visited = IndexSet::new();
            let cycles = self.find_raw_type_paths(lk, ctx, (lib, unit, parameter.raw()), t, declaration, &mut visited);
            all_cycles.extend(cycles);
        }
        for cycle in all_cycles {
            for parameter in cycle {
                let ast = unit_ast(lk, parameter.0, parameter.1);
                let p = ast.cast::<TypeParameter>(parameter.2).unwrap();
                if let Some(bound) = ast.get(p).bound {
                    self.bound_overrides
                        .insert((parameter.0, parameter.1, bound.raw()), LType::Built(TypeId::DYNAMIC));
                }
            }
        }
    }

    /// `_findRawTypePathsToDeclaration`: the paths are the type parameter
    /// nodes of the path.
    fn find_raw_type_paths(
        &self,
        lk: &Linker<'_>,
        ctx: &Ctx<'_>,
        start_parameter: NodeKey,
        start_type: LType,
        end: ElementId,
        visited: &mut IndexSet<ElementId>,
    ) -> Vec<Vec<NodeKey>> {
        let mut paths = Vec::new();
        let LType::Builder(b) = start_type else {
            return paths;
        };
        match &self.builders[b as usize].kind {
            BuilderKind::Named {
                element, arguments, ..
            } => {
                let Some(declaration) = *element else {
                    return paths;
                };
                if arguments.is_empty() {
                    if declaration == end {
                        paths.push(vec![start_parameter]);
                    } else if visited.insert(declaration) {
                        let parameters: Vec<EId<TypeParameterElement>> = match declaration.tag() {
                            Tag::Class | Tag::Enum | Tag::Mixin | Tag::ExtensionType => {
                                ctx.instance(EId::from_raw(declaration)).type_params.clone()
                            }
                            Tag::TypeAlias => ctx.get(EId::<TypeAliasElement>::from_raw(declaration)).type_params.clone(),
                            _ => Vec::new(),
                        };
                        for parameter in parameters {
                            let first = ctx.get(parameter).first_fragment().raw();
                            let Some(&(l, u, node)) = lk.core.fragment_nodes.get(&first) else {
                                continue;
                            };
                            let ast = unit_ast(lk, l as u32, u as u32);
                            let Some(p) = ast.cast::<TypeParameter>(node) else { continue };
                            let Some(bound) = ast.get(p).bound else { continue };
                            let Some(bt) = self.default_node_type((l as u32, u as u32, bound.raw())) else {
                                continue;
                            };
                            let tails = self.find_raw_type_paths(lk, ctx, (l as u32, u as u32, node), bt, end, visited);
                            for tail in tails {
                                let mut path = vec![start_parameter];
                                path.extend(tail);
                                paths.push(path);
                            }
                        }
                        visited.shift_remove(&declaration);
                    }
                } else {
                    for &argument in arguments {
                        paths.extend(self.find_raw_type_paths(lk, ctx, start_parameter, argument, end, visited));
                    }
                }
            }
            BuilderKind::Function {
                type_params,
                params,
                return_type,
                ..
            } => {
                paths.extend(self.find_raw_type_paths(lk, ctx, start_parameter, *return_type, end, visited));
                for &tp in type_params {
                    if let Some(bound) = self.bound_of(ctx, tp) {
                        paths.extend(self.find_raw_type_paths(lk, ctx, start_parameter, bound, end, visited));
                    }
                }
                for p in params {
                    paths.extend(self.find_raw_type_paths(lk, ctx, start_parameter, p.ty, end, visited));
                }
            }
            BuilderKind::Record { .. } => {}
        }
        paths
    }

    /// `_computeDefaultType`.
    fn compute_default_type(
        &mut self,
        lk: &Linker<'_>,
        ctx: &Ctx<'_>,
        lib: u32,
        unit: u32,
        list: Id<TypeParameterList>,
        graph_edges: bool,
    ) {
        let ast = unit_ast(lk, lib, unit);
        let nodes = ast.list(ast.get(list).type_parameters);
        let mut elements = Vec::new();
        let mut bounds: Vec<LType> = Vec::new();
        for &node in nodes {
            let Some(e) = declared_element(lk, (lib, unit, node.raw())).and_then(|e| e.cast::<TypeParameterElement>()) else {
                return;
            };
            elements.push(e);
            let bound = ast
                .get(node)
                .bound
                .and_then(|b| self.default_node_type((lib, unit, b.raw())))
                .unwrap_or(LType::Built(TypeId::DYNAMIC));
            bounds.push(bound);
        }
        let length = elements.len();
        // _TypeParametersGraph
        let mut edges: Vec<Vec<usize>> = vec![Vec::new(); length];
        if graph_edges {
            for i in 0..length {
                self.collect_references(ctx, &elements, i, Some(bounds[i]), &mut edges);
            }
        }
        let components = strong_components(length, &edges);
        for component in components {
            let mut upper: IndexMap<EId<TypeParameterElement>, LType> = IndexMap::new();
            let mut lower: IndexMap<EId<TypeParameterElement>, LType> = IndexMap::new();
            for &i in &component {
                upper.insert(elements[i], LType::Built(TypeId::DYNAMIC));
                lower.insert(elements[i], LType::Built(TypeId::NEVER));
            }
            for &i in &component {
                let variance = type_parameter_variance(ctx, elements[i]);
                if let Some(r) = self.replace_upper_lower(ctx, bounds[i], &upper, &lower, variance) {
                    bounds[i] = r;
                }
            }
        }
        for i in 0..length {
            let mut upper = IndexMap::new();
            let mut lower = IndexMap::new();
            upper.insert(elements[i], bounds[i]);
            lower.insert(elements[i], LType::Built(TypeId::NEVER));
            for j in 0..length {
                let variance = type_parameter_variance(ctx, elements[j]);
                if let Some(r) = self.replace_upper_lower(ctx, bounds[j], &upper, &lower, variance) {
                    bounds[j] = r;
                }
            }
        }
        for i in 0..length {
            self.pending_defaults.insert(elements[i], bounds[i]);
        }
    }

    /// `_TypeParametersGraph._collectReferencesFrom`.
    fn collect_references(&self, ctx: &Ctx<'_>, parameters: &[EId<TypeParameterElement>], index: usize, t: Option<LType>, edges: &mut [Vec<usize>]) {
        let Some(t) = t else { return };
        match t {
            LType::Builder(b) => match &self.builders[b as usize].kind {
                BuilderKind::Function {
                    type_params,
                    params,
                    return_type,
                    ..
                } => {
                    for &tp in type_params {
                        self.collect_references(ctx, parameters, index, self.bound_of(ctx, tp), edges);
                    }
                    for p in params {
                        self.collect_references(ctx, parameters, index, Some(p.ty), edges);
                    }
                    self.collect_references(ctx, parameters, index, Some(*return_type), edges);
                }
                BuilderKind::Named { arguments, .. } => {
                    for &a in arguments {
                        self.collect_references(ctx, parameters, index, Some(a), edges);
                    }
                }
                BuilderKind::Record { .. } => {}
            },
            LType::Built(t) => {
                if let TypeKind::TypeParameter { param, .. } = *ctx.ty(t)
                    && let Some(type_index) = parameters.iter().position(|&p| p == param)
                {
                    edges[type_index].push(index);
                }
            }
        }
    }

    /// `_UpperLowerReplacementVisitor.run`: `None` when nothing changes.
    fn replace_upper_lower(
        &mut self,
        ctx: &Ctx<'_>,
        t: LType,
        upper: &IndexMap<EId<TypeParameterElement>, LType>,
        lower: &IndexMap<EId<TypeParameterElement>, LType>,
        variance: Variance,
    ) -> Option<LType> {
        match t {
            LType::Built(id) => self.replace_built(ctx, id, upper, lower, variance),
            LType::Builder(b) => {
                let kind = self.builders[b as usize].kind.clone();
                match kind {
                    BuilderKind::Named {
                        element,
                        arguments,
                        nullability,
                        node,
                    } => {
                        // visitNamedTypeBuilder: the arguments with the
                        // variance of the type parameters of the element.
                        let parameters: Vec<EId<TypeParameterElement>> = match element {
                            Some(e) if matches!(e.tag(), Tag::Class | Tag::Enum | Tag::Mixin | Tag::Extension | Tag::ExtensionType) => {
                                ctx.instance(EId::from_raw(e)).type_params.clone()
                            }
                            Some(e) if e.tag() == Tag::TypeAlias => {
                                ctx.get(EId::<TypeAliasElement>::from_raw(e)).type_params.clone()
                            }
                            _ => Vec::new(),
                        };
                        let mut new_arguments: Option<Vec<LType>> = None;
                        for (i, &argument) in arguments.iter().enumerate() {
                            // Dart `_typeArguments`: arguments without a
                            // matching parameter keep their variance.
                            let v = match parameters.get(i) {
                                Some(&p) => variance_combine(variance, type_parameter_variance(ctx, p)),
                                None => variance,
                            };
                            if let Some(r) = self.replace_upper_lower(ctx, argument, upper, lower, v) {
                                new_arguments.get_or_insert_with(|| arguments.clone())[i] = r;
                            }
                        }
                        let new_arguments = new_arguments?;
                        Some(self.add_builder(BuilderKind::Named {
                            element,
                            arguments: new_arguments,
                            nullability,
                            node,
                        }))
                    }
                    BuilderKind::Function {
                        type_params,
                        params,
                        return_type,
                        nullability,
                        node,
                    } => {
                        // visitFunctionTypeBuilder: a type parameter whose
                        // bound changes is replaced by a fresh copy with the
                        // new bound, and the old type parameters are
                        // substituted by the new ones.
                        let mut new_type_params: Option<Vec<EId<TypeParameterElement>>> = None;
                        let mut new_bounds: Vec<Option<LType>> = vec![None; type_params.len()];
                        for (i, &tp) in type_params.iter().enumerate() {
                            if let Some(bound) = self.bound_of(ctx, tp)
                                && let Some(new_bound) = self.replace_upper_lower(ctx, bound, upper, lower, variance)
                            {
                                new_type_params.get_or_insert_with(|| type_params.clone())[i] = ctx.fresh_copy(tp);
                                new_bounds[i] = Some(new_bound);
                            }
                        }
                        let mut substitution: Option<Vec<(EId<TypeParameterElement>, TypeId)>> = None;
                        if let Some(new_type_params) = &new_type_params {
                            let map: Vec<(EId<TypeParameterElement>, TypeId)> = type_params
                                .iter()
                                .zip(new_type_params.iter())
                                .map(|(&old, &new)| (old, ctx.type_parameter_type(new, Nullability::None)))
                                .collect();
                            for (i, &new) in new_type_params.iter().enumerate() {
                                // freshCopy() keeps the bound; `..bound =`
                                // replaces it with the visited bound.
                                let bound = match new_bounds[i] {
                                    Some(b) => Some(b),
                                    None => self.bound_of(ctx, type_params[i]),
                                };
                                if let Some(bound) = bound {
                                    let bound = self.substitute_ltype(ctx, bound, &map);
                                    match bound {
                                        LType::Built(t) => ctx.get(new).bound.set(Some(t)),
                                        LType::Builder(_) => {
                                            ctx.get(new).bound.set(None);
                                            self.pending_bounds.insert(new, bound);
                                        }
                                    }
                                }
                            }
                            substitution = Some(map);
                        }
                        let mut changed = false;
                        let flipped = match variance {
                            Variance::Covariant => Variance::Contravariant,
                            Variance::Contravariant => Variance::Covariant,
                            v => v,
                        };
                        // visitType: the visited type (or the type itself),
                        // substituted when there are new type parameters.
                        let visit_type = |this: &mut Self, t: LType, v: Variance| -> Option<LType> {
                            let result = this.replace_upper_lower(ctx, t, upper, lower, v);
                            match &substitution {
                                Some(map) => Some(this.substitute_ltype(ctx, result.unwrap_or(t), map)),
                                None => result,
                            }
                        };
                        let new_return = match visit_type(self, return_type, variance) {
                            Some(r) => {
                                changed = true;
                                r
                            }
                            None => return_type,
                        };
                        let mut new_params = params.clone();
                        for p in new_params.iter_mut() {
                            if let Some(r) = visit_type(self, p.ty, flipped) {
                                p.ty = r;
                                changed = true;
                            }
                        }
                        // createFunctionTypeBuilder: new type parameters
                        // alone do not make a new builder.
                        if !changed {
                            return None;
                        }
                        Some(self.add_builder(BuilderKind::Function {
                            type_params: new_type_params.unwrap_or(type_params),
                            params: new_params,
                            return_type: new_return,
                            nullability,
                            node,
                        }))
                    }
                    BuilderKind::Record {
                        node,
                        positional,
                        named,
                        nullability,
                    } => {
                        let mut changed = false;
                        let mut p2 = positional.clone();
                        for t in p2.iter_mut() {
                            if let Some(r) = self.replace_upper_lower(ctx, *t, upper, lower, variance) {
                                *t = r;
                                changed = true;
                            }
                        }
                        let mut n2 = named.clone();
                        for (_, t) in n2.iter_mut() {
                            if let Some(r) = self.replace_upper_lower(ctx, *t, upper, lower, variance) {
                                *t = r;
                                changed = true;
                            }
                        }
                        if !changed {
                            return None;
                        }
                        Some(self.add_builder(BuilderKind::Record {
                            node,
                            positional: p2,
                            named: n2,
                            nullability,
                        }))
                    }
                }
            }
        }
    }

    /// `Substitution.fromMap(map).substituteType(t)` over a linking type:
    /// built types are substituted, builders are copied when something
    /// changes.
    fn substitute_ltype(&mut self, ctx: &Ctx<'_>, t: LType, map: &[(EId<TypeParameterElement>, TypeId)]) -> LType {
        match t {
            LType::Built(id) => {
                let (params, args): (Vec<_>, Vec<_>) = map.iter().copied().unzip();
                LType::Built(dartr_typesystem::MapSubstitution::from_pairs(&params, &args).substitute_type(ctx, id))
            }
            LType::Builder(b) => {
                let kind = self.builders[b as usize].kind.clone();
                let new_kind = match kind {
                    BuilderKind::Named {
                        element,
                        arguments,
                        nullability,
                        node,
                    } => {
                        let new_arguments: Vec<LType> = arguments.iter().map(|&a| self.substitute_ltype(ctx, a, map)).collect();
                        if new_arguments == arguments {
                            return t;
                        }
                        BuilderKind::Named {
                            element,
                            arguments: new_arguments,
                            nullability,
                            node,
                        }
                    }
                    BuilderKind::Function {
                        type_params,
                        params,
                        return_type,
                        nullability,
                        node,
                    } => {
                        let mut new_params = params.clone();
                        for p in new_params.iter_mut() {
                            p.ty = self.substitute_ltype(ctx, p.ty, map);
                        }
                        let new_return = self.substitute_ltype(ctx, return_type, map);
                        if new_return == return_type && new_params.iter().zip(params.iter()).all(|(a, b)| a.ty == b.ty) {
                            return t;
                        }
                        BuilderKind::Function {
                            type_params,
                            params: new_params,
                            return_type: new_return,
                            nullability,
                            node,
                        }
                    }
                    BuilderKind::Record {
                        node,
                        positional,
                        named,
                        nullability,
                    } => {
                        let new_positional: Vec<LType> = positional.iter().map(|&a| self.substitute_ltype(ctx, a, map)).collect();
                        let new_named: Vec<(Name, LType)> =
                            named.iter().map(|&(n, a)| (n, self.substitute_ltype(ctx, a, map))).collect();
                        if new_positional == positional && new_named == named {
                            return t;
                        }
                        BuilderKind::Record {
                            node,
                            positional: new_positional,
                            named: new_named,
                            nullability,
                        }
                    }
                };
                self.add_builder(new_kind)
            }
        }
    }

    /// The replacement of a built type: a type parameter type is replaced
    /// by the upper or lower value (Dart `visitTypeParameterType` returns
    /// the value as it is); other built types are handled by the
    /// `dartr_typesystem` replacement visitor when every value is built.
    fn replace_built(
        &mut self,
        ctx: &Ctx<'_>,
        id: TypeId,
        upper: &IndexMap<EId<TypeParameterElement>, LType>,
        lower: &IndexMap<EId<TypeParameterElement>, LType>,
        variance: Variance,
    ) -> Option<LType> {
        if let TypeKind::TypeParameter { param, .. } = *ctx.ty(id) {
            let map = if variance == Variance::Contravariant { lower } else { upper };
            return map.get(&param).copied();
        }
        let all_built = upper.values().chain(lower.values()).all(|v| matches!(v, LType::Built(_)));
        if !all_built {
            return None;
        }
        let built = |m: &IndexMap<EId<TypeParameterElement>, LType>| -> IndexMap<EId<TypeParameterElement>, TypeId> {
            m.iter()
                .map(|(k, v)| (*k, match v { LType::Built(t) => *t, LType::Builder(_) => unreachable!() }))
                .collect()
        };
        let mut visitor = UpperLowerVisitor {
            ctx: *ctx,
            upper: built(upper),
            lower: built(lower),
            variance,
        };
        dartr_typesystem::replacement_visitor::ReplacementVisitor::visit(&mut visitor, id).map(LType::Built)
    }
}

/// `_UpperLowerReplacementVisitor` over built types.
struct UpperLowerVisitor<'a> {
    ctx: Ctx<'a>,
    upper: IndexMap<EId<TypeParameterElement>, TypeId>,
    lower: IndexMap<EId<TypeParameterElement>, TypeId>,
    variance: Variance,
}

impl<'a> dartr_typesystem::replacement_visitor::ReplacementVisitor<'a> for UpperLowerVisitor<'a> {
    fn ctx(&self) -> Ctx<'a> {
        self.ctx
    }

    fn change_variance(&mut self) {
        self.variance = match self.variance {
            Variance::Covariant => Variance::Contravariant,
            Variance::Contravariant => Variance::Covariant,
            v => v,
        };
    }

    fn visit_type_argument(&mut self, parameter: EId<TypeParameterElement>, argument: TypeId) -> Option<TypeId> {
        let saved = self.variance;
        self.variance = variance_combine(self.variance, type_parameter_variance(&self.ctx, parameter));
        let r = dartr_typesystem::replacement_visitor::ReplacementVisitor::visit(self, argument);
        self.variance = saved;
        r
    }

    fn visit_type_parameter_type(&mut self, t: TypeId) -> Option<TypeId> {
        let TypeKind::TypeParameter { param, .. } = *self.ctx.ty(t) else {
            return None;
        };
        if self.variance == Variance::Contravariant {
            self.lower.get(&param).copied()
        } else {
            self.upper.get(&param).copied()
        }
    }
}

/// `computeStrongComponents` (Tarjan, components in reverse topological
/// order of the edges, as the Dart utility returns them).
pub fn strong_components(n: usize, edges: &[Vec<usize>]) -> Vec<Vec<usize>> {
    struct State<'e> {
        edges: &'e [Vec<usize>],
        index: Vec<i64>,
        low: Vec<i64>,
        on_stack: Vec<bool>,
        stack: Vec<usize>,
        next: i64,
        result: Vec<Vec<usize>>,
    }
    fn connect(s: &mut State<'_>, v: usize) {
        s.index[v] = s.next;
        s.low[v] = s.next;
        s.next += 1;
        s.stack.push(v);
        s.on_stack[v] = true;
        for &w in &s.edges[v] {
            if s.index[w] < 0 {
                connect(s, w);
                s.low[v] = s.low[v].min(s.low[w]);
            } else if s.on_stack[w] {
                s.low[v] = s.low[v].min(s.index[w]);
            }
        }
        if s.low[v] == s.index[v] {
            let mut component = Vec::new();
            loop {
                let w = s.stack.pop().unwrap();
                s.on_stack[w] = false;
                component.push(w);
                if w == v {
                    break;
                }
            }
            s.result.push(component);
        }
    }
    let mut s = State {
        edges,
        index: vec![-1; n],
        low: vec![0; n],
        on_stack: vec![false; n],
        stack: Vec::new(),
        next: 0,
        result: Vec::new(),
    };
    for v in 0..n {
        if s.index[v] < 0 {
            connect(&mut s, v);
        }
    }
    s.result
}

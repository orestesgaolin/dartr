// Dart source: pkg/analyzer/lib/src/summary2/types_builder.dart
// (TypesBuilder, _MixinsInference), pkg/analyzer/lib/src/summary2/
// interface_cycles.dart, pkg/analyzer/lib/src/summary2/link.dart
// (Linker._resolveTypes, _createTypeSystem),
// pkg/analyzer/lib/src/dart/element/type_provider.dart (the slots)

//! The last part of type resolution: the types of the declarations
//! (`TypesBuilder._declaration`), mixin inference, and breaking interface
//! cycles; and [`resolve_types`], the order of `Linker._resolveTypes`.
//!
//! Mixin inference without explicit type arguments of a generic mixin needs
//! the generic inferrer (unit A6, not ported yet): such a mixin keeps its
//! default type arguments.

use dartr_ast::NamedType as NamedTypeNode;
use dartr_ast::*;
use dartr_element::*;
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_typesystem::{TypeExt, TypeSystem};
use indexmap::{IndexMap, IndexSet};

use crate::ast_util::*;
use crate::link::Linker;
use crate::scope::LibraryScopes;
use crate::types::*;

/// A context for the cycle being linked.
pub fn link_ctx<'a>(lk: &'a Linker<'_>, tp: &'a TypeProvider, features: &'a FeatureSet) -> Ctx<'a> {
    Ctx {
        world: lk.core.world,
        current: Some(&lk.core.store),
        local: None,
        tp,
        features,
        req: &NoopSink,
    }
}

/// Dart `elementFactory.createTypeProviders(dartCore, dartAsync)`: the
/// classes of `dart:core` and `dart:async` by name.
pub fn create_type_provider(lk: &Linker<'_>) -> TypeProvider {
    let library = |uri: &str| -> Option<EId<LibraryElement>> {
        if let Some(&b) = lk.builder_by_uri.get(uri) {
            return Some(lk.builders[b].element);
        }
        lk.core.world.libraries.get(uri).copied()
    };
    type_provider_for(lk.core.world, Some(&lk.core.store), library("dart:core"), library("dart:async"))
}

/// A [`TypeProvider`] for the world (the libraries of linked cycles).
pub fn world_type_provider(world: &WorldSnapshot) -> TypeProvider {
    type_provider_for(
        world,
        None,
        world.libraries.get("dart:core").copied(),
        world.libraries.get("dart:async").copied(),
    )
}

fn type_provider_for(
    world: &WorldSnapshot,
    current: Option<&ElementStore>,
    core: Option<EId<LibraryElement>>,
    async_: Option<EId<LibraryElement>>,
) -> TypeProvider {
    let tp = TypeProvider::default();
    let features = FeatureSet::default();
    let (Some(core), Some(async_)) = (core, async_) else {
        return tp;
    };
    {
        let ctx = Ctx {
            world,
            current,
            local: None,
            tp: &tp,
            features: &features,
            req: &NoopSink,
        };
        let class = |library: EId<LibraryElement>, name: &str| -> Option<EId<ClassElement>> {
            ctx.get(library)
                .classes
                .iter()
                .copied()
                .find(|&c| ctx.get(c).name.map(|n| ctx.name_str(n)) == Some(name))
        };
        tp.core_library.set_once(core);
        tp.async_library.set_once(async_);
        macro_rules! slot {
            ($slot:ident, $lib:expr, $name:expr) => {
                if let Some(c) = class($lib, $name) {
                    tp.$slot.set_once(c);
                }
            };
        }
        slot!(bool_element, core, "bool");
        slot!(deprecated_element, core, "Deprecated");
        slot!(double_element, core, "double");
        slot!(function_element, core, "Function");
        slot!(future_element, async_, "Future");
        slot!(future_or_element, async_, "FutureOr");
        slot!(int_element, core, "int");
        slot!(iterable_element, core, "Iterable");
        slot!(list_element, core, "List");
        slot!(map_element, core, "Map");
        slot!(null_element, core, "Null");
        slot!(num_element, core, "num");
        slot!(object_element, core, "Object");
        slot!(record_element, core, "Record");
        slot!(set_element, core, "Set");
        slot!(stack_trace_element, core, "StackTrace");
        slot!(stream_element, async_, "Stream");
        slot!(string_element, core, "String");
        slot!(symbol_element, core, "Symbol");
        slot!(type_element, core, "Type");
        let enum_element = class(core, "Enum");
        tp.enum_element.set_once(enum_element);
        let instantiate = |c: EId<ClassElement>, args: &[TypeId], n: Nullability| {
            ctx.interface_type(c.upcast(), args, n)
        };
        tp.enum_type
            .set_once(enum_element.map(|e| instantiate(e, &[], Nullability::None)));
        macro_rules! ty {
            ($slot:ident, $el:ident, $args:expr, $n:expr) => {
                if let Some(&c) = tp.$el.try_get() {
                    tp.$slot.set_once(instantiate(c, $args, $n));
                }
            };
        }
        use Nullability::{None as N, Question as Q};
        ty!(bool_type, bool_element, &[], N);
        ty!(deprecated_type, deprecated_element, &[], N);
        ty!(double_type, double_element, &[], N);
        ty!(double_type_question, double_element, &[], Q);
        ty!(function_type, function_element, &[], N);
        ty!(future_dynamic_type, future_element, &[TypeId::DYNAMIC], N);
        ty!(int_type, int_element, &[], N);
        ty!(int_type_question, int_element, &[], Q);
        ty!(iterable_dynamic_type, iterable_element, &[TypeId::DYNAMIC], N);
        ty!(null_type, null_element, &[], N);
        ty!(num_type, num_element, &[], N);
        ty!(num_type_question, num_element, &[], Q);
        ty!(object_type, object_element, &[], N);
        ty!(object_question_type, object_element, &[], Q);
        ty!(record_type, record_element, &[], N);
        ty!(stack_trace_type, stack_trace_element, &[], N);
        ty!(stream_dynamic_type, stream_element, &[TypeId::DYNAMIC], N);
        ty!(string_type, string_element, &[], N);
        ty!(symbol_type, symbol_element, &[], N);
        ty!(type_type, type_element, &[], N);
        if let (Some(&f), Some(&null)) = (tp.future_element.try_get(), tp.null_type.try_get()) {
            tp.future_null_type.set_once(instantiate(f, &[null], N));
        }
        if let (Some(&f), Some(&null)) = (tp.future_or_element.try_get(), tp.null_type.try_get()) {
            tp.future_or_null_type.set_once(instantiate(f, &[null], N));
        }
        if let (Some(&i), Some(&o)) = (tp.iterable_element.try_get(), tp.object_type.try_get()) {
            tp.iterable_object_type.set_once(instantiate(i, &[o], N));
        }
        if let (Some(&m), Some(&o)) = (tp.map_element.try_get(), tp.object_type.try_get()) {
            tp.map_object_object_type.set_once(instantiate(m, &[o, o], N));
        }
    }
    tp
}

/// Dart `Linker._resolveTypes`.
pub fn resolve_types(lk: &mut Linker<'_>, tp: &TypeProvider) {
    let features = FeatureSet::default();
    let mut tr = TypeResolution::default();
    {
        let lk_ref: &Linker<'_> = lk;
        let scopes = LibraryScopes::build(lk_ref);
        let ctx = link_ctx(lk_ref, tp, &features);
        for (lib, builder) in lk_ref.builders.iter().enumerate() {
            for unit in 0..builder.units.len() {
                ReferenceResolver::new(&mut tr, lk_ref, &ctx, &scopes, lib as u32, unit as u32).resolve_unit();
            }
        }
    }
    let variances = crate::type_bounds::compute_variances(lk, tp, &tr);
    for (p, v) in variances {
        lk.core.store.get_mut(p).variance = Some(v);
    }
    let lk_ref: &Linker<'_> = lk;
    let ctx = link_ctx(lk_ref, tp, &features);
    crate::type_bounds::compute_simply_bounded(lk_ref, &ctx, &tr);
    crate::type_bounds::find_type_alias_self_references(lk_ref, &ctx, &tr);
    // TypesBuilder.build
    tr.build_default_types(lk_ref, &ctx);
    let builders = tr.type_builders.clone();
    for b in builders {
        tr.build(lk_ref, &ctx, b);
    }
    let mut to_infer_mixins: IndexMap<ElementId, Vec<NodeKey>> = IndexMap::new();
    let declarations = tr.declarations.clone();
    for &key in &declarations {
        declaration(&mut tr, lk_ref, &ctx, key, &mut to_infer_mixins);
    }
    mixins_inference(&mut tr, lk_ref, &ctx, &to_infer_mixins);
    break_interface_cycles(lk_ref, &ctx, &declarations);
}

/// Dart `isInterfaceTypeInterface`.
pub fn is_interface_type_interface(ctx: &Ctx<'_>, t: TypeId) -> bool {
    let TypeKind::Interface {
        element,
        nullability,
        ..
    } = *ctx.ty(t)
    else {
        return false;
    };
    if matches!(element.raw().tag(), Tag::Enum | Tag::ExtensionType) {
        return false;
    }
    if ctx.is_dart_core_function(t) || ctx.is_dart_core_null(t) {
        return false;
    }
    if nullability == Nullability::Question {
        return false;
    }
    true
}

/// Dart `_isInterfaceTypeClass`.
fn is_interface_type_class(ctx: &Ctx<'_>, t: TypeId) -> bool {
    let TypeKind::Interface { element, .. } = *ctx.ty(t) else {
        return false;
    };
    element.raw().tag() == Tag::Class && is_interface_type_interface(ctx, t)
}

/// The built type of a node.
fn built(tr: &mut TypeResolution, lk: &Linker<'_>, ctx: &Ctx<'_>, key: NodeKey) -> Option<TypeId> {
    let t = tr.node_type(key)?;
    Some(tr.build_type(lk, ctx, t))
}

/// Dart `_toInterfaceTypeList`.
fn to_interface_type_list(
    tr: &mut TypeResolution,
    lk: &Linker<'_>,
    ctx: &Ctx<'_>,
    lib: u32,
    unit: u32,
    list: Option<NodeList<NamedTypeNode>>,
) -> Vec<TypeId> {
    let Some(list) = list else { return Vec::new() };
    let ast = unit_ast(lk, lib, unit);
    ast.list(list)
        .iter()
        .filter_map(|n| built(tr, lk, ctx, (lib, unit, n.raw())))
        .filter(|&t| is_interface_type_interface(ctx, t))
        .collect()
}

fn append_interfaces(ctx: &Ctx<'_>, element: EId<InterfaceElement>, more: Vec<TypeId>) {
    let i = ctx.interface(element);
    let mut all: Vec<TypeId> = ctx.list(i.interfaces.get().unwrap_or(TypeList::EMPTY)).to_vec();
    all.extend(more);
    i.interfaces.set(Some(ctx.intern_list(&all)));
}

/// Dart `PropertyInducingElementImpl.type =` (also the types of the
/// synthetic accessors).
pub fn set_variable_type(ctx: &Ctx<'_>, variable: EId<PropertyInducingElement>, t: TypeId) {
    let v = ctx.property_inducing(variable);
    v.type_.set(Some(t));
    if let Some(getter) = v.getter
        && ctx
            .fragment_data(ctx.get(getter).first_fragment)
            .unwrap()
            .flags
            .has(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE)
    {
        set_return_type(ctx, getter.upcast(), t);
    }
    if let Some(setter) = v.setter
        && ctx
            .fragment_data(ctx.get(setter).first_fragment)
            .unwrap()
            .flags
            .has(FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE)
    {
        set_return_type(ctx, setter.upcast(), TypeId::VOID);
        if let Some(&value) = ctx.get(setter).formal_params.first() {
            ctx.get(value).type_.set(Some(t));
        }
    }
}

/// Dart `ExecutableElementImpl.returnType =` (resets the cached type).
pub fn set_return_type(ctx: &Ctx<'_>, e: EId<ExecutableElement>, t: TypeId) {
    let x = ctx.executable(e);
    x.return_type.set(Some(t));
    x.type_.set(None);
}

fn first_fragment_previous(ctx: &Ctx<'_>, f: FragmentId) -> bool {
    ctx.fragment_data(f).unwrap().previous_fragment.is_some()
}

/// Dart `TypesBuilder._buildFunctionType` (with the formal parameter
/// elements of the nodes).
fn build_function_type(
    tr: &mut TypeResolution,
    lk: &Linker<'_>,
    ctx: &Ctx<'_>,
    lib: u32,
    unit: u32,
    type_parameters: Option<Id<TypeParameterList>>,
    return_type: Option<Id<TypeAnnotation>>,
    formal_parameters: Id<FormalParameterList>,
    nullability: Nullability,
) -> TypeId {
    let ret = return_type
        .and_then(|r| built(tr, lk, ctx, (lib, unit, r.raw())))
        .unwrap_or(TypeId::DYNAMIC);
    let ast = unit_ast(lk, lib, unit);
    let tps: Vec<EId<TypeParameterElement>> = match type_parameters {
        Some(list) => ast
            .list(ast.get(list).type_parameters)
            .iter()
            .filter_map(|tp| declared_element(lk, (lib, unit, tp.raw())).and_then(|e| e.cast()))
            .collect(),
        None => Vec::new(),
    };
    let params: Vec<FnParam> = ast
        .list(ast.get(formal_parameters).parameters)
        .iter()
        .filter_map(|p| declared_element(lk, (lib, unit, p.raw())))
        .map(|e| {
            let p = ctx.get(EId::<FormalParameterElement>::from_raw(e));
            FnParam {
                name: p.name,
                kind: p.kind,
                ty: p.type_.get().unwrap_or(TypeId::INVALID),
                covariant: p.flags.has(ElementFlags::FORMAL_PARAMETER_ELEMENT_IS_COVARIANT),
                element: Some(ElemRef::Base(e)),
            }
        })
        .collect();
    ctx.function_type(&tps, &params, ret, nullability, None)
}

/// Dart `TypesBuilder._declaration`.
fn declaration(
    tr: &mut TypeResolution,
    lk: &Linker<'_>,
    ctx: &Ctx<'_>,
    key: NodeKey,
    to_infer_mixins: &mut IndexMap<ElementId, Vec<NodeKey>>,
) {
    let (lib, unit, node) = key;
    let ast = unit_ast(lk, lib, unit);
    let fragment = declared_fragment(lk, key);
    let element = declared_element(lk, key);
    if let Some(n) = ast.cast::<ClassDeclaration>(node) {
        let n = ast.get(n);
        let Some(element) = element else { return };
        let i = EId::<InterfaceElement>::from_raw(element);
        if ctx.interface(i).supertype.get().is_none()
            && let Some(e) = n.extends_clause
            && let Some(t) = built(tr, lk, ctx, (lib, unit, ast.get(e).superclass.raw()))
            && is_interface_type_class(ctx, t)
        {
            ctx.interface(i).supertype.set(Some(t));
        }
        let more = to_interface_type_list(tr, lk, ctx, lib, unit, n.implements_clause.map(|c| ast.get(c).interfaces));
        append_interfaces(ctx, i, more);
        if let Some(w) = n.with_clause {
            to_infer_mixins.entry(element).or_default().push((lib, unit, w.raw()));
        }
    } else if let Some(n) = ast.cast::<ClassTypeAlias>(node) {
        let n = ast.get(n);
        let Some(element) = element else { return };
        let i = EId::<InterfaceElement>::from_raw(element);
        if let Some(t) = built(tr, lk, ctx, (lib, unit, n.superclass.raw()))
            && is_interface_type_class(ctx, t)
        {
            ctx.interface(i).supertype.set(Some(t));
        }
        let mixins = to_interface_type_list(tr, lk, ctx, lib, unit, Some(ast.get(n.with_clause).mixin_types));
        ctx.interface(i).mixins.set(Some(ctx.intern_list(&mixins)));
        let interfaces = to_interface_type_list(tr, lk, ctx, lib, unit, n.implements_clause.map(|c| ast.get(c).interfaces));
        ctx.interface(i).interfaces.set(Some(ctx.intern_list(&interfaces)));
        to_infer_mixins.entry(element).or_default().push((lib, unit, n.with_clause.raw()));
    } else if let Some(n) = ast.cast::<EnumDeclaration>(node) {
        let n = ast.get(n);
        let Some(element) = element else { return };
        let i = EId::<InterfaceElement>::from_raw(element);
        let more = to_interface_type_list(tr, lk, ctx, lib, unit, n.implements_clause.map(|c| ast.get(c).interfaces));
        append_interfaces(ctx, i, more);
        if let Some(w) = n.with_clause {
            to_infer_mixins.entry(element).or_default().push((lib, unit, w.raw()));
        }
    } else if let Some(n) = ast.cast::<ExtensionDeclaration>(node) {
        if let Some(on) = ast.get(n).on_clause
            && let Some(f) = fragment
            && !first_fragment_previous(ctx, f)
            && let Some(element) = element
        {
            let t = built(tr, lk, ctx, (lib, unit, ast.get(on).extended_type.raw())).unwrap_or(TypeId::INVALID);
            ctx.get(EId::<ExtensionElement>::from_raw(element)).extended_type.set(Some(t));
        }
    } else if let Some(n) = ast.cast::<ExtensionTypeDeclaration>(node) {
        let Some(element) = element else { return };
        if let Some(c) = ast.get(n).implements_clause {
            let ts = TypeSystem::new(*ctx);
            let interfaces: Vec<TypeId> = ast
                .list(ast.get(c).interfaces)
                .iter()
                .filter_map(|t| built(tr, lk, ctx, (lib, unit, t.raw())))
                .filter(|&t| matches!(ctx.ty(t), TypeKind::Interface { .. }))
                .filter(|&t| ts.is_valid_extension_type_superinterface(t))
                .collect();
            append_interfaces(ctx, EId::from_raw(element), interfaces);
        }
    } else if ast.is::<FieldFormalParameter>(node) || ast.is::<SuperFormalParameter>(node) || ast.is::<RegularFormalParameter>(node) {
        let Some(element) = element else { return };
        let p = EId::<FormalParameterElement>::from_raw(element);
        let fp = Id::<FormalParameter>::from_raw(node);
        let (_, _, suffix) = crate::informative_data::formal_parameter_parts(ast, fp);
        let type_node = formal_parameter_type_node(ast, fp);
        if let Some(s) = suffix {
            let s = ast.get(s);
            let n = if s.question.is_some() {
                Nullability::Question
            } else {
                Nullability::None
            };
            let t = build_function_type(tr, lk, ctx, lib, unit, s.type_parameters, type_node, s.formal_parameters, n);
            ctx.get(p).type_.set(Some(t));
            return;
        }
        match type_node {
            None => {
                if ast.is::<RegularFormalParameter>(node)
                    && element.tag() == Tag::FieldFormalParameter
                    && ctx
                        .fragment_data(ctx.get(p).first_fragment)
                        .unwrap()
                        .flags
                        .has(FragmentFlags::FIELD_FORMAL_PARAMETER_FRAGMENT_IS_DECLARING)
                {
                    return;
                }
                ctx.get(p).type_.set(Some(TypeId::DYNAMIC));
            }
            Some(t) => {
                let t = built(tr, lk, ctx, (lib, unit, t.raw())).unwrap_or(TypeId::INVALID);
                ctx.get(p).type_.set(Some(t));
            }
        }
    } else if let Some(n) = ast.cast::<FunctionDeclaration>(node) {
        let Some(f) = fragment else { return };
        if first_fragment_previous(ctx, f) {
            return;
        }
        let Some(element) = element else { return };
        let return_type = match ast.get(n).return_type {
            Some(r) => built(tr, lk, ctx, (lib, unit, r.raw())).unwrap_or(TypeId::INVALID),
            None if function_is_setter(ast, n) => TypeId::VOID,
            None => TypeId::DYNAMIC,
        };
        set_return_type(ctx, EId::from_raw(element), return_type);
        set_synthetic_variable_type(ctx, element);
    } else if let Some(n) = ast.cast::<FunctionTypeAlias>(node) {
        let Some(element) = element else { return };
        let n = ast.get(n);
        let t = build_function_type(tr, lk, ctx, lib, unit, None, n.return_type, n.parameters, Nullability::None);
        ctx.get(EId::<TypeAliasElement>::from_raw(element)).aliased_type.set(Some(t));
    } else if let Some(n) = ast.cast::<GenericFunctionType>(node) {
        let Some(element) = element else { return };
        let t = ast
            .get(n)
            .return_type
            .and_then(|r| built(tr, lk, ctx, (lib, unit, r.raw())))
            .unwrap_or(TypeId::DYNAMIC);
        ctx.get(EId::<GenericFunctionTypeElement>::from_raw(element)).return_type.set(Some(t));
    } else if let Some(n) = ast.cast::<GenericTypeAlias>(node) {
        let Some(element) = element else { return };
        let type_node = ast.get(n).type_;
        let t = if lk.builders[lib as usize].is_enabled(ExperimentalFlag::NonfunctionTypeAliases)
            || ast.is::<GenericFunctionType>(type_node.raw())
        {
            built(tr, lk, ctx, (lib, unit, type_node.raw())).unwrap_or(TypeId::INVALID)
        } else {
            ctx.function_type(&[], &[], TypeId::DYNAMIC, Nullability::None, None)
        };
        ctx.get(EId::<TypeAliasElement>::from_raw(element)).aliased_type.set(Some(t));
    } else if let Some(n) = ast.cast::<MethodDeclaration>(node) {
        let Some(f) = fragment else { return };
        if first_fragment_previous(ctx, f) {
            return;
        }
        let Some(element) = element else { return };
        let n2 = ast.get(n);
        let return_type = match n2.return_type {
            Some(r) => built(tr, lk, ctx, (lib, unit, r.raw())).unwrap_or(TypeId::INVALID),
            None if method_is_setter(ast, n) => TypeId::VOID,
            None if n2.operator_keyword.is_some() && ast.tokens.lexeme(n2.name) == "[]=" => TypeId::VOID,
            None => TypeId::DYNAMIC,
        };
        set_return_type(ctx, EId::from_raw(element), return_type);
        set_synthetic_variable_type(ctx, element);
    } else if let Some(n) = ast.cast::<MixinDeclaration>(node) {
        let Some(element) = element else { return };
        let n = ast.get(n);
        let m = ctx.get(EId::<MixinElement>::from_raw(element));
        let mut constraints: Vec<TypeId> = ctx.list(m.superclass_constraints.get().unwrap_or(TypeList::EMPTY)).to_vec();
        constraints.extend(to_interface_type_list(tr, lk, ctx, lib, unit, n.on_clause.map(|c| ast.get(c).superclass_constraints)));
        m.superclass_constraints.set(Some(ctx.intern_list(&constraints)));
        let more = to_interface_type_list(tr, lk, ctx, lib, unit, n.implements_clause.map(|c| ast.get(c).interfaces));
        append_interfaces(ctx, EId::from_raw(element), more);
    } else if let Some(n) = ast.cast::<TypeParameter>(node) {
        let Some(f) = fragment else { return };
        if first_fragment_previous(ctx, f) {
            return;
        }
        let Some(element) = element else { return };
        let p = EId::<TypeParameterElement>::from_raw(element);
        let bound = ast.get(n).bound.and_then(|b| built(tr, lk, ctx, (lib, unit, b.raw())));
        tr.pending_bounds.shift_remove(&p);
        ctx.get(p).bound.set(bound);
    } else if let Some(n) = ast.cast::<VariableDeclarationList>(node) {
        let n = ast.get(n);
        let Some(type_node) = n.type_ else { return };
        let Some(t) = built(tr, lk, ctx, (lib, unit, type_node.raw())) else { return };
        for &v in ast.list(n.variables) {
            let Some(f) = declared_fragment(lk, (lib, unit, v.raw())) else { continue };
            if first_fragment_previous(ctx, f) {
                continue;
            }
            let Some(&e) = ctx.fragment_data(f).unwrap().element.try_get() else { continue };
            set_variable_type(ctx, EId::from_raw(e), t);
        }
    }
}

/// Dart `_setSyntheticVariableType`.
fn set_synthetic_variable_type(ctx: &Ctx<'_>, element: ElementId) {
    let origin_getter_setter = |v: EId<PropertyInducingElement>| {
        ctx.fragment_data(ctx.element_data(v.raw()).unwrap().first_fragment)
            .unwrap()
            .flags
            .has(FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_GETTER_SETTER)
    };
    match element.tag() {
        Tag::Getter => {
            let g = ctx.get(EId::<GetterElement>::from_raw(element));
            if let Some(v) = g.variable.get()
                && origin_getter_setter(v)
            {
                ctx.property_inducing(v).type_.set(g.return_type.get());
            }
        }
        Tag::Setter => {
            let s = ctx.get(EId::<SetterElement>::from_raw(element));
            if let Some(v) = s.variable.get()
                && origin_getter_setter(v)
                && ctx.property_inducing(v).getter.is_none()
            {
                let value_type = s
                    .formal_params
                    .first()
                    .and_then(|&p| ctx.get(p).type_.get());
                ctx.property_inducing(v).type_.set(value_type);
            }
        }
        _ => {}
    }
}

/// Dart `_MixinsInference.perform`. The type arguments of a generic mixin
/// without explicit type arguments are inferred by matching its superclass
/// constraints with the supertypes structurally; Dart uses the generic
/// inferrer (`matchSupertypeConstraints`, unit A6), which gives the same
/// result when each type parameter appears directly in a constraint.
/// Otherwise the mixin keeps its default type arguments.
fn mixins_inference(
    tr: &mut TypeResolution,
    lk: &Linker<'_>,
    ctx: &Ctx<'_>,
    declarations: &IndexMap<ElementId, Vec<NodeKey>>,
) {
    use dartr_typesystem::class_hierarchy::InterfacesMerger;
    for (&element, withs) in declarations {
        let i = ctx.interface(EId::from_raw(element));
        let mut merger = InterfacesMerger::new(TypeSystem::new(*ctx));
        merger.add_with_supertypes(i.supertype.get());
        let mut mixins = Vec::new();
        for &(lib, unit, with) in withs {
            let ast = unit_ast(lk, lib, unit);
            let Some(w) = ast.cast::<WithClause>(with) else { continue };
            for &m in ast.list(ast.get(w).mixin_types) {
                let Some(t) = built(tr, lk, ctx, (lib, unit, m.raw())) else { continue };
                if !is_interface_type_interface(ctx, t) {
                    continue;
                }
                let t = if ast.get(m).type_arguments.is_none() {
                    let node_element = match tr.node_elements.get(&(lib, unit, m.raw())) {
                        Some(Some(crate::scope::ScopeElement::Element(e))) => Some(*e),
                        _ => None,
                    };
                    infer_mixin(ctx, &merger, node_element, t).unwrap_or(t)
                } else {
                    t
                };
                mixins.push(t);
                merger.add_with_supertypes(Some(t));
            }
        }
        i.mixins.set(Some(ctx.intern_list(&mixins)));
    }
}

/// Dart `_MixinInference._inferSingle` for a mixin without type arguments.
fn infer_mixin(
    ctx: &Ctx<'_>,
    merger: &dartr_typesystem::class_hierarchy::InterfacesMerger<'_>,
    element: Option<ElementId>,
    mixin_type: TypeId,
) -> Option<TypeId> {
    let element = element?;
    let nullability = ctx.nullability_suffix(mixin_type);
    let (type_parameters, constraints): (Vec<EId<TypeParameterElement>>, Vec<TypeId>) =
        if let Some(i) = element.cast::<InterfaceElement>() {
            let tps = ctx.instance(i.upcast()).type_params.clone();
            if tps.is_empty() {
                return None;
            }
            let constraints = TypeSystem::new(*ctx).gather_mixin_supertype_constraints_for_inference(i);
            (tps, constraints)
        } else {
            let a = element.cast::<TypeAliasElement>()?;
            let tps = ctx.get(a).type_params.clone();
            if tps.is_empty() {
                return None;
            }
            let raw = ctx.get(a).aliased_type.get()?;
            let TypeKind::Interface { element: e, .. } = *ctx.ty(raw) else {
                return None;
            };
            let substitution = dartr_typesystem::MapSubstitution::from_interface_type(ctx, raw);
            let constraints: Vec<TypeId> = ctx
                .element_superclass_constraints(e)
                .iter()
                .map(|&c| substitution.substitute_type(ctx, c))
                .collect();
            (tps, constraints)
        };
    let candidates = merger.type_list();
    let mut solution: IndexMap<EId<TypeParameterElement>, TypeId> = IndexMap::new();
    for &constraint in &constraints {
        let TypeKind::Interface { element: ce, .. } = *ctx.ty(constraint) else {
            return None;
        };
        let matching = candidates
            .iter()
            .copied()
            .find(|&t| ctx.interface_element(t) == Some(ce))?;
        if !unify(ctx, &type_parameters, constraint, matching, &mut solution) {
            return None;
        }
    }
    let args: Vec<TypeId> = type_parameters
        .iter()
        .map(|p| solution.get(p).copied())
        .collect::<Option<Vec<_>>>()?;
    if let Some(i) = element.cast::<InterfaceElement>() {
        Some(ctx.interface_type(i, &args, nullability))
    } else {
        Some(ctx.instantiate_type_alias(EId::from_raw(element), &args, nullability))
    }
}

/// Structural matching of a constraint with a supertype: binds the type
/// parameters of [params].
fn unify(
    ctx: &Ctx<'_>,
    params: &[EId<TypeParameterElement>],
    pattern: TypeId,
    t: TypeId,
    solution: &mut IndexMap<EId<TypeParameterElement>, TypeId>,
) -> bool {
    match (*ctx.ty(pattern), *ctx.ty(t)) {
        (TypeKind::TypeParameter { param, nullability: Nullability::None, .. }, _) if params.contains(&param) => {
            match solution.get(&param) {
                Some(&existing) => existing == t,
                None => {
                    solution.insert(param, t);
                    true
                }
            }
        }
        (
            TypeKind::Interface { element: e1, args: a1, .. },
            TypeKind::Interface { element: e2, args: a2, .. },
        ) if e1 == e2 => {
            let a1 = ctx.list(a1).to_vec();
            let a2 = ctx.list(a2).to_vec();
            a1.len() == a2.len() && a1.iter().zip(a2.iter()).all(|(&x, &y)| unify(ctx, params, x, y, solution))
        }
        _ => pattern == t,
    }
}

/// Dart `breakInterfaceCycles`.
fn break_interface_cycles(lk: &Linker<'_>, ctx: &Ctx<'_>, declarations: &[NodeKey]) {
    let mut elements = Vec::new();
    for &key in declarations {
        if let Some(e) = declared_element(lk, key)
            && matches!(e.tag(), Tag::Class | Tag::Enum | Tag::Mixin | Tag::ExtensionType)
        {
            elements.push(EId::<InterfaceElement>::from_raw(e));
        }
    }
    let mut walker = ImplementsWalker {
        ctx,
        nodes: IndexMap::new(),
        index: 1,
        stack: Vec::new(),
    };
    for e in elements {
        walker.walk(e);
    }
}

struct WalkNode {
    index: u32,
    low_link: u32,
    evaluated: bool,
    dependencies: Option<Vec<EId<InterfaceElement>>>,
}

/// Dart `_ImplementsWalker` (a `DependencyWalker`).
struct ImplementsWalker<'c, 'a> {
    ctx: &'c Ctx<'a>,
    nodes: IndexMap<EId<InterfaceElement>, WalkNode>,
    index: u32,
    stack: Vec<EId<InterfaceElement>>,
}

impl ImplementsWalker<'_, '_> {
    fn node(&mut self, e: EId<InterfaceElement>) -> &mut WalkNode {
        self.nodes.entry(e).or_insert(WalkNode {
            index: 0,
            low_link: 0,
            evaluated: false,
            dependencies: None,
        })
    }

    fn dependencies(&mut self, e: EId<InterfaceElement>) -> Vec<EId<InterfaceElement>> {
        if let Some(d) = &self.node(e).dependencies {
            return d.clone();
        }
        let ctx = self.ctx;
        let i = ctx.interface(e);
        let mut types: Vec<TypeId> = Vec::new();
        types.extend(i.supertype.get());
        types.extend(ctx.list(i.mixins.get().unwrap_or(TypeList::EMPTY)));
        types.extend(ctx.list(i.interfaces.get().unwrap_or(TypeList::EMPTY)));
        if e.raw().tag() == Tag::Mixin {
            types.extend(ctx.list(
                ctx.get(EId::<MixinElement>::from_raw(e.raw()))
                    .superclass_constraints
                    .get()
                    .unwrap_or(TypeList::EMPTY),
            ));
        }
        let deps: Vec<EId<InterfaceElement>> = types
            .into_iter()
            .filter_map(|t| match *ctx.ty(t) {
                TypeKind::Interface { element, .. } => Some(element),
                _ => None,
            })
            .collect();
        self.node(e).dependencies = Some(deps.clone());
        deps
    }

    fn walk(&mut self, e: EId<InterfaceElement>) {
        if self.node(e).evaluated {
            return;
        }
        self.strong_connect(e);
    }

    fn strong_connect(&mut self, e: EId<InterfaceElement>) {
        let mut has_trivial_cycle = false;
        let index = self.index;
        self.index += 1;
        {
            let n = self.node(e);
            n.index = index;
            n.low_link = index;
        }
        self.stack.push(e);
        for d in self.dependencies(e) {
            if self.node(d).evaluated {
                continue;
            }
            if d == e {
                has_trivial_cycle = true;
            } else if self.node(d).index == 0 {
                self.strong_connect(d);
                let low = self.node(d).low_link;
                let n = self.node(e);
                n.low_link = n.low_link.min(low);
            } else {
                let di = self.node(d).index;
                let n = self.node(e);
                n.low_link = n.low_link.min(di);
            }
        }
        let n = self.node(e);
        if n.low_link == n.index {
            if *self.stack.last().unwrap() == e {
                self.stack.pop();
                if has_trivial_cycle {
                    self.mark_circular(&[e]);
                } else {
                    self.node(e).evaluated = true;
                }
            } else {
                let mut scc = Vec::new();
                loop {
                    let other = self.stack.pop().unwrap();
                    scc.push(other);
                    if other == e {
                        break;
                    }
                }
                self.mark_circular(&scc);
            }
        }
    }

    /// Dart `_markCircular` for each node of [scc].
    fn mark_circular(&mut self, scc: &[EId<InterfaceElement>]) {
        let ctx = self.ctx;
        for &e in scc {
            self.node(e).evaluated = true;
            let i = ctx.interface(e);
            if !i.interface_cycle.is_set() {
                i.interface_cycle.set_once(scc.to_vec());
            }
            let object = ctx.tp.object_type.try_get().copied();
            match e.raw().tag() {
                Tag::Class => {
                    i.supertype.set(object);
                    i.mixins.set(Some(TypeList::EMPTY));
                    i.interfaces.set(Some(TypeList::EMPTY));
                }
                Tag::Enum => {
                    i.mixins.set(Some(TypeList::EMPTY));
                    i.interfaces.set(Some(TypeList::EMPTY));
                }
                Tag::ExtensionType => {
                    let et = ctx.get(EId::<ExtensionTypeElement>::from_raw(e.raw()));
                    et.has_implements_self_reference.set(true);
                    let ts = TypeSystem::new(*ctx);
                    let representation = crate::type_bounds::representation_type(ctx, EId::from_raw(e.raw()));
                    let superinterface = if ts.is_non_nullable(representation) {
                        ts.object_none()
                    } else {
                        ts.object_question()
                    };
                    i.interfaces.set(Some(ctx.intern_list(&[superinterface])));
                }
                Tag::Mixin => {
                    let m = ctx.get(EId::<MixinElement>::from_raw(e.raw()));
                    m.superclass_constraints
                        .set(Some(ctx.intern_list(&object.into_iter().collect::<Vec<_>>())));
                    i.interfaces.set(Some(TypeList::EMPTY));
                }
                _ => {}
            }
        }
    }
}

#[allow(dead_code)]
fn unused(_: IndexSet<u32>) {}

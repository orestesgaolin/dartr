// Dart source: pkg/analyzer/lib/src/summary2/link.dart
// (_computeHasNonFinalField, _setDefaultSupertypes, _buildEnumChildren,
// _computeFieldPromotability), pkg/analyzer/lib/src/summary2/library_builder.dart
// (setDefaultSupertypes, buildEnumChildren, _FieldPromotability),
// pkg/_fe_analyzer_shared/lib/src/field_promotability.dart,
// pkg/analyzer/lib/src/summary2/types_builder.dart
// (_copyDeclaringFormalParametersExplicitTypes),
// pkg/analyzer/lib/src/summary2/extension_type.dart,
// pkg/analyzer/lib/src/summary2/super_constructor_resolver.dart,
// pkg/analyzer/lib/src/summary2/instance_member_inferrer.dart
// (_setInducedModifier), pkg/analyzer/lib/src/dart/element/element.dart
// (ClassElementImpl._buildMixinAppConstructors)

//! Unit B4 and the structural parts of the later outline phases that need
//! the resolved supertypes.

use dartr_element::*;
use dartr_parser::experimental_flags::ExperimentalFlag;
use dartr_typesystem::type_algebra::MapSubstitution;
use dartr_typesystem::{TypeExt, TypeSystem};
use indexmap::{IndexMap, IndexSet};

use crate::link::Linker;
use crate::types_builder::{link_ctx, set_variable_type};

fn interfaces_of(ctx: &Ctx<'_>, library: EId<LibraryElement>) -> Vec<EId<InterfaceElement>> {
    let l = ctx.get(library);
    l.classes
        .iter()
        .map(|e| e.upcast())
        .chain(l.enums.iter().map(|e| e.upcast()))
        .chain(l.extension_types.iter().map(|e| e.upcast()))
        .chain(l.mixins.iter().map(|e| e.upcast()))
        .collect()
}

fn first_has(ctx: &Ctx<'_>, e: ElementId, flag: FragmentFlags) -> bool {
    let first = ctx.element_data(e).unwrap().first_fragment;
    ctx.fragment_data(first).unwrap().flags.has(flag)
}

fn interface_element_of(ctx: &Ctx<'_>, t: TypeId) -> Option<EId<InterfaceElement>> {
    match *ctx.ty(t) {
        TypeKind::Interface { element, .. } => Some(element),
        _ => None,
    }
}

/// Dart `ClassElementImpl.isFinal` / `isBase` / `isInterface` (the element
/// flags) of any interface element.
fn class_flag(ctx: &Ctx<'_>, e: EId<InterfaceElement>, flag: ElementFlags) -> bool {
    match e.raw().tag() {
        Tag::Class => ctx.element_data(e.raw()).unwrap().flags.has(flag),
        Tag::Mixin if flag == ElementFlags::CLASS_ELEMENT_IS_BASE => {
            first_has(ctx, e.raw(), FragmentFlags::MIXIN_FRAGMENT_IS_BASE)
        }
        _ => false,
    }
}

/// The phases after `_resolveTypes`, in the order of `_buildOutlines`.
pub fn build_outlines(lk: &mut Linker<'_>, tp: &TypeProvider, resolver: &dyn crate::link::LinkResolver) {
    let features = FeatureSet::default();
    {
        let lk_ref: &Linker<'_> = lk;
        let ctx = link_ctx(lk_ref, tp, &features);
        copy_declaring_formal_parameters_explicit_types(lk_ref, &ctx);
        compute_has_non_final_field(lk_ref, &ctx);
        set_default_supertypes(lk_ref, &ctx);
    }
    for index in 0..lk.builders.len() {
        crate::library_builder::LibraryBuilder::build_class_synthetic_constructors(lk, index);
    }
    for index in 0..lk.builders.len() {
        crate::library_builder::LibraryBuilder::build_enum_synthetic_constructors(lk, index);
    }
    for index in 0..lk.builders.len() {
        crate::library_builder::LibraryBuilder::replace_const_fields_if_no_const_constructor(lk, index);
    }
    for index in 0..lk.builders.len() {
        crate::library_builder::LibraryBuilder::resolve_constructor_field_formals(lk, index);
    }
    {
        let lk_ref: &Linker<'_> = lk;
        let ctx = link_ctx(lk_ref, tp, &features);
        build_enum_children(lk_ref, &ctx);
        for index in 0..lk_ref.builders.len() {
            compute_field_promotability(lk_ref, &ctx, index);
        }
    }
    complete_classes(lk, tp);
    // The elements built since `_computeLibraryScopes` (synthetic and mixin
    // application constructors) get their `enclosingElement` and `library`:
    // top-level inference resolves initializers that read them (Dart
    // computes both from the fragments on demand).
    crate::link::set_library_and_enclosing(&mut lk.core.store);
    let lk_ref: &Linker<'_> = lk;
    let ctx = link_ctx(lk_ref, tp, &features);
    // _performTopLevelInference
    crate::top_level_inference::infer(lk_ref, &ctx, resolver);
    build_extension_types(lk_ref, &ctx);
    // _resolveConstructors
    resolve_redirected_constructors(lk_ref, &ctx);
}

/// Dart `TypesBuilder._copyDeclaringFormalParametersExplicitTypes`.
fn copy_declaring_formal_parameters_explicit_types(lk: &Linker<'_>, ctx: &Ctx<'_>) {
    for &(field, formal) in &lk.core.declaring_formal_parameters {
        let Some(&formal_element) = ctx.fragment(formal).element.try_get() else {
            continue;
        };
        let has_implicit_type = first_has(ctx, formal_element, FragmentFlags::VARIABLE_FRAGMENT_HAS_IMPLICIT_TYPE);
        if has_implicit_type {
            continue;
        }
        let Some(&field_element) = ctx.fragment(field).element.try_get() else {
            continue;
        };
        if let Some(t) = ctx.get(EId::<FormalParameterElement>::from_raw(formal_element)).type_.get() {
            set_variable_type(ctx, EId::from_raw(field_element), t);
        }
    }
}

/// Dart `Linker._computeHasNonFinalField`.
fn compute_has_non_final_field(lk: &Linker<'_>, ctx: &Ctx<'_>) {
    let mut linking: IndexSet<EId<InterfaceElement>> = IndexSet::new();
    for b in &lk.builders {
        for e in interfaces_of(ctx, b.element) {
            linking.insert(e);
        }
    }
    let mut computed: IndexSet<EId<InterfaceElement>> = IndexSet::new();
    fn compute_for(
        ctx: &Ctx<'_>,
        linking: &IndexSet<EId<InterfaceElement>>,
        computed: &mut IndexSet<EId<InterfaceElement>>,
        e: EId<InterfaceElement>,
    ) -> bool {
        let i = ctx.interface(e);
        if !linking.contains(&e) || !computed.insert(e) {
            return i.has_non_final_field.get();
        }
        let mut types: Vec<TypeId> = Vec::new();
        types.extend(i.supertype.get());
        types.extend(ctx.list(i.mixins.get().unwrap_or(TypeList::EMPTY)));
        let mut result = false;
        for t in types {
            if let Some(s) = interface_element_of(ctx, t)
                && compute_for(ctx, linking, computed, s)
            {
                result = true;
            }
        }
        for &f in &i.fields {
            let is_static = first_has(ctx, f.raw(), FragmentFlags::VARIABLE_FRAGMENT_IS_STATIC);
            if is_static {
                continue;
            }
            let origin_declaration = first_has(ctx, f.raw(), FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_DECLARATION);
            let is_abstract = first_has(ctx, f.raw(), FragmentFlags::VARIABLE_FRAGMENT_IS_ABSTRACT);
            let declaring = first_has(ctx, f.raw(), FragmentFlags::FIELD_FRAGMENT_IS_ORIGIN_DECLARING_FORMAL_PARAMETER);
            if (origin_declaration && !is_abstract) || declaring {
                let is_final = first_has(ctx, f.raw(), FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL);
                let is_const = first_has(ctx, f.raw(), FragmentFlags::VARIABLE_FRAGMENT_IS_CONST);
                if !(is_final || is_const) {
                    result = true;
                }
            }
        }
        i.has_non_final_field.set(result);
        result
    }
    for &e in &linking {
        compute_for(ctx, &linking, &mut computed, e);
    }
}

/// Dart `LibraryBuilder.setDefaultSupertypes`.
fn set_default_supertypes(lk: &Linker<'_>, ctx: &Ctx<'_>) {
    let Some(&object) = ctx.tp.object_type.try_get() else {
        return;
    };
    for b in &lk.builders {
        let l = ctx.get(b.element);
        for &c in &l.classes {
            let is_object = &*b.uri == "dart:core" && ctx.get(c).name.map(|n| ctx.name_str(n)) == Some("Object");
            if !is_object && ctx.get(c).supertype.get().is_none() {
                ctx.get(c).supertype.set(Some(object));
            }
        }
        for &m in &l.mixins {
            let mixin = ctx.get(m);
            if mixin.superclass_constraints.get().unwrap_or(TypeList::EMPTY).is_empty() {
                mixin.superclass_constraints.set(Some(ctx.intern_list(&[object])));
            }
        }
    }
}

/// The classes and enums of the cycle, completed superclass first: the
/// constructors of a mixin application (Dart
/// `ClassElementImpl._buildMixinAppConstructors`, built on first access in
/// Dart, here before the store is frozen), the super constructors (Dart
/// `SuperConstructorResolver`) and the types of initializing formal
/// parameters (Dart `InstanceMemberInferrer._inferConstructor`, the part
/// that does not need top-level inference). The super constructor is
/// recorded as the base element (Dart: the substituted member).
fn complete_classes(lk: &mut Linker<'_>, tp: &TypeProvider) {
    let mut classes: Vec<(usize, EId<InterfaceElement>)> = Vec::new();
    for (index, b) in lk.builders.iter().enumerate() {
        let l = lk.core.store.get(b.element);
        for &c in &l.classes {
            classes.push((index, c.upcast()));
        }
        for &e in &l.enums {
            classes.push((index, e.upcast()));
        }
    }
    let mut done: IndexSet<EId<InterfaceElement>> = IndexSet::new();
    for &(index, c) in &classes.clone() {
        complete_class(lk, tp, &classes, &mut done, index, c);
    }
}

fn complete_class(
    lk: &mut Linker<'_>,
    tp: &TypeProvider,
    classes: &[(usize, EId<InterfaceElement>)],
    done: &mut IndexSet<EId<InterfaceElement>>,
    index: usize,
    class: EId<InterfaceElement>,
) {
    if !done.insert(class) {
        return;
    }
    let features = FeatureSet::default();
    let superclass = {
        let ctx = link_ctx(lk, tp, &features);
        ctx.interface(class).supertype.get().and_then(|t| interface_element_of(&ctx, t))
    };
    if let Some(s) = superclass
        && let Some(&(i, _)) = classes.iter().find(|(_, x)| *x == s)
    {
        complete_class(lk, tp, classes, done, i, s);
    }
    let is_mixin_application = class.raw().tag() == Tag::Class
        && lk
            .core
            .store
            .fragment_data(lk.core.store.element_data(class.raw()).unwrap().first_fragment)
            .unwrap()
            .flags
            .has(FragmentFlags::CLASS_FRAGMENT_IS_MIXIN_APPLICATION);
    if is_mixin_application {
        build_mixin_app(lk, tp, index, EId::from_raw(class.raw()));
        return;
    }
    let ctx = link_ctx(lk, tp, &features);
    resolve_super_constructors_of(lk, &ctx, class);
}

/// The type of Dart `SuperFormalParameterElementImpl.superConstructorParameter`
/// (substituted with the supertype).
pub fn super_constructor_parameter_type(
    ctx: &Ctx<'_>,
    class: EId<InterfaceElement>,
    constructor: EId<ConstructorElement>,
    p: EId<FormalParameterElement>,
) -> Option<TypeId> {
    let positional_super_formals: Vec<EId<FormalParameterElement>> = ctx
        .get(constructor)
        .formal_params
        .iter()
        .copied()
        .filter(|p| p.raw().tag() == Tag::SuperFormalParameter && ctx.get(*p).kind.is_positional())
        .collect();
    let ElemRef::Base(super_constructor) = ctx.get(constructor).super_constructor.get()? else {
        return None;
    };
    let super_params = &ctx.get(EId::<ConstructorElement>::from_raw(super_constructor)).formal_params;
    let pe = ctx.get(p);
    let found = if pe.kind.is_named() {
        super_params
            .iter()
            .copied()
            .find(|&s| ctx.get(s).kind.is_named() && ctx.get(s).name == pe.name)
    } else {
        let index = positional_super_formals.iter().position(|&x| x == p)?;
        super_params
            .iter()
            .copied()
            .filter(|&s| ctx.get(s).kind.is_positional())
            .nth(index)
    }?;
    let t = ctx.get(found).type_.get()?;
    let supertype = ctx.interface(class).supertype.get()?;
    let substitution = MapSubstitution::from_interface_type(ctx, supertype);
    Some(substitution.substitute_type(ctx, t))
}

fn build_mixin_app(lk: &mut Linker<'_>, tp: &TypeProvider, index: usize, class: EId<ClassElement>) {
    let features = FeatureSet::default();
    let Some(super_type) = lk.core.store.get(class).supertype.get() else {
        return;
    };
    struct NewParam {
        name: Option<Name>,
        kind: ParameterKind,
        ty: TypeId,
        is_const: bool,
        is_final: bool,
        initializer: Option<(StoreId, ConstExprId)>,
    }
    struct NewCtor {
        name: Option<Name>,
        is_const: bool,
        super_constructor: ElementId,
        params: Vec<NewParam>,
    }
    let (ctors, class_name, first, has_type_params) = {
        let ctx = link_ctx(lk, tp, &features);
        let Some(super_element) = interface_element_of(&ctx, super_type) else {
            return;
        };
        let library = lk.builders[index].element;
        let substitution = MapSubstitution::from_interface_type(&ctx, super_type);
        let mixins: Vec<TypeId> = ctx
            .list(ctx.get(class).mixins.get().unwrap_or(TypeList::EMPTY))
            .to_vec();
        let type_has_instance_variables = |t: TypeId| -> bool {
            let Some(e) = interface_element_of(&ctx, t) else {
                return false;
            };
            ctx.interface(e).fields.iter().any(|&f| {
                !first_has(&ctx, f.raw(), FragmentFlags::VARIABLE_FRAGMENT_IS_STATIC)
                    && first_has(&ctx, f.raw(), FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_DECLARATION)
            })
        };
        let any_instance_variables = mixins.iter().any(|&m| type_has_instance_variables(m));
        let mut ctors = Vec::new();
        for &sc in &ctx.interface(super_element).constructors {
            let s = ctx.get(sc);
            let name_text = s.name.map(|n| ctx.name_str(n)).unwrap_or("");
            // Dart `isAccessibleIn(library)`.
            let accessible = !name_text.starts_with('_')
                || ctx.element_data(sc.raw()).unwrap().library == Some(library);
            if !accessible {
                continue;
            }
            if first_has(&ctx, sc.raw(), FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY) {
                continue;
            }
            let is_const = first_has(&ctx, sc.raw(), FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_CONST) && !any_instance_variables;
            let mut params = Vec::new();
            for &p in &s.formal_params {
                let pe = ctx.get(p);
                let first = pe.first_fragment();
                let f = ctx.fragment(first);
                params.push(NewParam {
                    name: f.name,
                    kind: f.parameter_kind,
                    ty: substitution.substitute_type(&ctx, pe.type_.get().unwrap_or(TypeId::INVALID)),
                    is_const: f.flags.has(FragmentFlags::VARIABLE_FRAGMENT_IS_CONST),
                    // Dart `isFinal` of the element: always `true` for
                    // field formal and super formal parameters.
                    is_final: matches!(p.raw().tag(), Tag::FieldFormalParameter | Tag::SuperFormalParameter)
                        || f.flags.has(FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL),
                    initializer: f.constant_initializer.map(|e| (first.store(), e)),
                });
            }
            ctors.push(NewCtor {
                name: s.name,
                is_const,
                super_constructor: sc.raw(),
                params,
            });
        }
        let c = ctx.get(class);
        (ctors, c.name, c.first_fragment(), !c.type_params.is_empty())
    };
    let new = lk.core.name("new");
    let mut elements = Vec::new();
    let mut fragments = Vec::new();
    for ctor in ctors {
        let name = ctor.name.unwrap_or(new);
        let mut data = crate::element_builder::new_constructor_fragment(FragmentData::new(Some(name), None));
        data.type_name = class_name;
        data.fragment.enclosing_fragment = Some(first.raw());
        data.flags
            .set(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_ORIGIN_MIXIN_APPLICATION, true);
        data.flags.set(FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_CONST, ctor.is_const);
        let fragment = lk.core.store.add_fragment::<ConstructorFragment>(data);
        let element = crate::library_builder::new_constructor_element(&mut lk.core, fragment, name);
        lk.core.store.get(element).flags.set(
            ElementFlags::EXECUTABLE_ELEMENT_HAS_ENCLOSING_TYPE_PARAMETER_REFERENCE,
            has_type_params,
        );
        lk.core
            .store
            .get(element)
            .super_constructor
            .set(Some(ElemRef::Base(ctor.super_constructor)));
        let mut param_fragments = Vec::new();
        let mut param_elements = Vec::new();
        for p in ctor.params {
            let mut data = FormalParameterFragment {
                variable: VariableFragmentData::new(FragmentData::new(p.name, None)),
                parameter_kind: p.kind,
                private_name: None,
            };
            data.constant_initializer = p.initializer.and_then(|(store, expr)| copy_const_expr(lk, store, expr));
            data.fragment.enclosing_fragment = Some(fragment.raw());
            data.flags.set(FragmentFlags::VARIABLE_FRAGMENT_IS_CONST, p.is_const);
            data.flags.set(FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL, p.is_final);
            data.flags.set(
                FragmentFlags::FORMAL_PARAMETER_FRAGMENT_IS_ORIGIN_MIXIN_APPLICATION_CLASS_CONSTRUCTOR,
                true,
            );
            let pf = lk.core.store.add_fragment::<FormalParameterFragment>(data);
            let pe = crate::element_builder::init_formal_parameter_element(&mut lk.core.store, pf);
            lk.core.store.get(pe).type_.set(Some(p.ty));
            param_fragments.push(pf);
            param_elements.push(pe);
        }
        lk.core.store.fragment_mut(fragment).formal_params = param_fragments;
        lk.core.store.get_mut(element).formal_params = param_elements;
        {
            let b = &mut lk.builders[index];
            if let Some(&container) = b.element_references.get(&class.raw()) {
                let key = lk.core.generation.names.get(name).to_string();
                let r = b.references.reference.get_or_create_member(
                    container,
                    crate::reference::MemberReferenceKind::Constructor,
                    &key,
                );
                b.references.reference.set_element(r, element.raw());
            }
        }
        elements.push(element);
        fragments.push(fragment);
    }
    lk.core.store.get_mut(class).constructors = elements;
    lk.core.store.fragment_mut(first).constructors = fragments;
}

/// Copies a const expression of [store] (this cycle or a linked cycle)
/// into the `ConstExprs` of this cycle.
fn copy_const_expr(lk: &mut Linker<'_>, store: StoreId, expr: ConstExprId) -> Option<ConstExprId> {
    if store == lk.core.store.id {
        let src = lk.core.const_exprs.ast.clone();
        return Some(lk.core.const_exprs.copy(&src, expr.0));
    }
    let exprs = lk.core.deps.const_exprs(store)?;
    Some(lk.core.const_exprs.copy(&exprs.ast, expr.0))
}

/// Dart `LibraryBuilder.buildEnumChildren`.
fn build_enum_children(lk: &Linker<'_>, ctx: &Ctx<'_>) {
    let ts = TypeSystem::new(*ctx);
    for b in &lk.builders {
        for implicit in b.implicit_enum_nodes.values() {
            let Some(&e) = ctx.fragment(implicit.fragment).element.try_get() else {
                continue;
            };
            let e = EId::<EnumElement>::from_raw(e);
            let supertype = ctx.tp.enum_type.try_get().copied().flatten().or(ctx.tp.object_type.try_get().copied());
            ctx.get(e).supertype.set(supertype);
            let Some(&list) = ctx.tp.list_element.try_get() else {
                continue;
            };
            let instance = ts.instantiate_interface_to_bounds(e.upcast(), Nullability::None);
            let values_type = ctx.interface_type(list.upcast(), &[instance], Nullability::None);
            if let Some(&values) = ctx.fragment(implicit.values_fragment).element.try_get() {
                set_variable_type(ctx, EId::from_raw(values), values_type);
            }
        }
    }
}

/// Dart `_FieldPromotability.perform` of one library.
fn compute_field_promotability(lk: &Linker<'_>, ctx: &Ctx<'_>, index: usize) {
    let builder = &lk.builders[index];
    let enabled = builder.is_enabled(ExperimentalFlag::InferenceUpdate2);
    let library = builder.element;
    let l = ctx.get(library);
    let mut fp = FieldPromotability::default();
    let mut potentially_promotable: Vec<EId<FieldElement>> = Vec::new();
    let mut handle = |fp: &mut FieldPromotability, class: EId<InterfaceElement>, is_abstract: bool| {
        let info = fp.add_class(class, is_abstract);
        let i = ctx.interface(class);
        for &field in &i.fields {
            if first_has(ctx, field.raw(), FragmentFlags::VARIABLE_FRAGMENT_IS_STATIC)
                || first_has(ctx, field.raw(), FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_GETTER_SETTER)
            {
                continue;
            }
            let Some(name) = ctx.get(field).name else { continue };
            let name = ctx.name_str(name);
            let ok = fp.add_field(
                info,
                field,
                name,
                first_has(ctx, field.raw(), FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL),
                first_has(ctx, field.raw(), FragmentFlags::VARIABLE_FRAGMENT_IS_ABSTRACT),
                first_has(ctx, field.raw(), FragmentFlags::VARIABLE_FRAGMENT_IS_EXTERNAL),
            );
            if enabled && ok {
                potentially_promotable.push(field);
            }
        }
        for &getter in &i.getters {
            if first_has(ctx, getter.raw(), FragmentFlags::EXECUTABLE_FRAGMENT_IS_STATIC)
                || first_has(ctx, getter.raw(), FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE)
            {
                continue;
            }
            let Some(name) = ctx.get(getter).name else { continue };
            let name = ctx.name_str(name);
            let ok = fp.add_getter(
                info,
                getter,
                name,
                first_has(ctx, getter.raw(), FragmentFlags::EXECUTABLE_FRAGMENT_IS_ABSTRACT),
            );
            if enabled
                && ok
                && let Some(v) = ctx.get(getter).variable.get()
                && let Some(f) = v.raw().cast::<FieldElement>()
            {
                potentially_promotable.push(f);
            }
        }
    };
    for &c in &l.classes {
        let is_abstract = ctx.element_data(c.raw()).unwrap().flags.has(ElementFlags::CLASS_ELEMENT_IS_ABSTRACT);
        handle(&mut fp, c.upcast(), is_abstract);
    }
    for &e in &l.enums {
        handle(&mut fp, e.upcast(), false);
    }
    for &m in &l.mixins {
        handle(&mut fp, m.upcast(), true);
    }
    for &et in &l.extension_types {
        if let Some(&representation) = ctx.get(et).fields.first()
            && let Some(name) = ctx.get(representation).name
            && ctx.name_str(name).starts_with('_')
        {
            ctx.fragment_data(ctx.get(representation).first_fragment)
                .unwrap()
                .flags
                .set(FragmentFlags::FIELD_FRAGMENT_IS_PROMOTABLE, true);
        }
    }
    let info = fp.compute_non_promotability_info(ctx);
    for field in potentially_promotable {
        let Some(name) = ctx.get(field).name else { continue };
        if !info.contains_key(ctx.name_str(name)) {
            ctx.fragment_data(ctx.get(field).first_fragment)
                .unwrap()
                .flags
                .set(FragmentFlags::FIELD_FRAGMENT_IS_PROMOTABLE, true);
        }
    }
    let map: IndexMap<Name, FieldNameNonPromotabilityInfo> = info
        .into_iter()
        .map(|(k, v)| (ctx.name(&k), v))
        .collect();
    let slot = &ctx.get(library).field_name_non_promotability_info;
    if !slot.is_set() {
        slot.set_once(map);
    }
}

/// Dart `FieldPromotability` (the shared algorithm).
#[derive(Default)]
struct FieldPromotability {
    info: IndexMap<String, FieldNameNonPromotabilityInfo>,
    interface_names: IndexMap<EId<InterfaceElement>, IndexSet<String>>,
    implemented_names: IndexMap<EId<InterfaceElement>, IndexSet<String>>,
    concrete: Vec<EId<InterfaceElement>>,
}

impl FieldPromotability {
    fn add_class(&mut self, class: EId<InterfaceElement>, is_abstract: bool) -> EId<InterfaceElement> {
        self.interface_names.entry(class).or_default();
        self.implemented_names.entry(class).or_default();
        if !is_abstract {
            self.concrete.push(class);
        }
        class
    }

    /// Dart `addField`: whether there is no non-promotability reason.
    fn add_field(&mut self, class: EId<InterfaceElement>, field: EId<FieldElement>, name: &str, is_final: bool, is_abstract: bool, is_external: bool) -> bool {
        if !name.starts_with('_') {
            return false;
        }
        self.interface_names.entry(class).or_default().insert(name.to_string());
        if !is_abstract {
            self.implemented_names.entry(class).or_default().insert(name.to_string());
        }
        if is_external || !is_final {
            self.info.entry(name.to_string()).or_default().conflicting_fields.push(field);
            return false;
        }
        true
    }

    /// Dart `addGetter`.
    fn add_getter(&mut self, class: EId<InterfaceElement>, getter: EId<GetterElement>, name: &str, is_abstract: bool) -> bool {
        if !name.starts_with('_') {
            return false;
        }
        self.interface_names.entry(class).or_default().insert(name.to_string());
        if !is_abstract {
            self.implemented_names.entry(class).or_default().insert(name.to_string());
            self.info
                .entry(name.to_string())
                .or_default()
                .conflicting_getters
                .push(getter.upcast());
            false
        } else {
            true
        }
    }

    /// Dart `_FieldPromotability.getSuperclasses`.
    fn superclasses(ctx: &Ctx<'_>, class: EId<InterfaceElement>, ignore_implements: bool) -> Vec<EId<InterfaceElement>> {
        let i = ctx.interface(class);
        let mut types: Vec<TypeId> = Vec::new();
        types.extend(i.supertype.get());
        types.extend(ctx.list(i.mixins.get().unwrap_or(TypeList::EMPTY)));
        if !ignore_implements {
            types.extend(ctx.list(i.interfaces.get().unwrap_or(TypeList::EMPTY)));
            if class.raw().tag() == Tag::Mixin {
                types.extend(ctx.list(
                    ctx.get(EId::<MixinElement>::from_raw(class.raw()))
                        .superclass_constraints
                        .get()
                        .unwrap_or(TypeList::EMPTY),
                ));
            }
        }
        types.into_iter().filter_map(|t| interface_element_of(ctx, t)).collect()
    }

    /// The transitive names of [class] (Dart `_ClassHierarchyWalker`; the
    /// union over the hierarchy is the same as the SCC computation).
    fn transitive_names(
        ctx: &Ctx<'_>,
        direct: &IndexMap<EId<InterfaceElement>, IndexSet<String>>,
        class: EId<InterfaceElement>,
        ignore_implements: bool,
    ) -> IndexSet<String> {
        let mut result = IndexSet::new();
        let mut visited = IndexSet::new();
        let mut stack = vec![class];
        while let Some(c) = stack.pop() {
            if !visited.insert(c) {
                continue;
            }
            if let Some(names) = direct.get(&c) {
                result.extend(names.iter().cloned());
            }
            stack.extend(Self::superclasses(ctx, c, ignore_implements));
        }
        result
    }

    /// Dart `computeNonPromotabilityInfo`.
    fn compute_non_promotability_info(mut self, ctx: &Ctx<'_>) -> IndexMap<String, FieldNameNonPromotabilityInfo> {
        let concrete = std::mem::take(&mut self.concrete);
        for class in concrete {
            let interface_names = Self::transitive_names(ctx, &self.interface_names, class, false);
            let implemented_names = Self::transitive_names(ctx, &self.implemented_names, class, true);
            for name in interface_names {
                if !implemented_names.contains(&name) {
                    self.info.entry(name).or_default().conflicting_nsm_classes.push(class);
                }
            }
        }
        self.info
    }
}

/// Dart `SuperConstructorResolver._constructor` for the constructors of
/// [interface].
fn resolve_super_constructors_of(lk: &Linker<'_>, ctx: &Ctx<'_>, interface: EId<InterfaceElement>) {
    use dartr_ast::*;
    let i = ctx.interface(interface);
    for &constructor in &i.constructors {
        if first_has(ctx, constructor.raw(), FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_FACTORY) {
            continue;
        }
        let named_constructor = |name: &str| -> Option<ElemRef> {
            let supertype = i.supertype.get()?;
            let s = interface_element_of(ctx, supertype)?;
            ctx.interface(s)
                .constructors
                .iter()
                .find(|&&c| ctx.get(c).name.map(|n| ctx.name_str(n)) == Some(name))
                .map(|c| ElemRef::Base(c.raw()))
        };
        let mut invokes_default = true;
        let mut fragment = Some(ctx.get(constructor).first_fragment().raw());
        while let Some(f) = fragment {
            if let Some(&(lib, unit, node)) = lk.core.fragment_nodes.get(&f) {
                let ast = crate::types::unit_ast(lk, lib as u32, unit as u32);
                if let Some(c) = ast.cast::<ConstructorDeclaration>(node) {
                    for &init in ast.list(ast.get(c).initializers) {
                        if ast.is::<RedirectingConstructorInvocation>(init.raw()) {
                            invokes_default = false;
                        } else if let Some(s) = ast.cast::<SuperConstructorInvocation>(init.raw()) {
                            invokes_default = false;
                            let name = ast
                                .get(s)
                                .constructor_name
                                .map(|n| ast.tokens.lexeme(ast.get(n).token))
                                .unwrap_or("new");
                            ctx.get(constructor).super_constructor.set(named_constructor(name));
                        }
                    }
                }
            }
            fragment = ctx.fragment_data(f).unwrap().next_fragment;
        }
        if invokes_default {
            ctx.get(constructor).super_constructor.set(named_constructor("new"));
        }
    }
}

/// Dart `InstanceMemberInferrer._setInducedModifier`.
pub fn set_induced_modifier(ctx: &Ctx<'_>, class: EId<InterfaceElement>) {
    if class.raw().tag() != Tag::Class {
        return;
    }
    let flags = &ctx.element_data(class.raw()).unwrap().flags;
    if !first_has(ctx, class.raw(), FragmentFlags::CLASS_FRAGMENT_IS_SEALED) {
        return;
    }
    let i = ctx.interface(class);
    let elements = |l: Option<TypeList>| -> Vec<EId<InterfaceElement>> {
        ctx.list(l.unwrap_or(TypeList::EMPTY))
            .iter()
            .filter_map(|&t| interface_element_of(ctx, t))
            .collect()
    };
    let mixins = elements(i.mixins.get());
    let interfaces = elements(i.interfaces.get());
    let is = |e: EId<InterfaceElement>, f: ElementFlags| class_flag(ctx, e, f);
    use ElementFlags as F;
    if mixins.iter().any(|&m| is(m, F::CLASS_ELEMENT_IS_FINAL)) {
        flags.set(F::CLASS_ELEMENT_IS_FINAL, true);
        return;
    }
    if let Some(s) = i.supertype.get().and_then(|t| interface_element_of(ctx, t)) {
        if is(s, F::CLASS_ELEMENT_IS_FINAL) {
            flags.set(F::CLASS_ELEMENT_IS_FINAL, true);
            return;
        }
        if is(s, F::CLASS_ELEMENT_IS_BASE) {
            if mixins.iter().any(|&m| is(m, F::CLASS_ELEMENT_IS_INTERFACE)) {
                flags.set(F::CLASS_ELEMENT_IS_FINAL, true);
                return;
            }
            flags.set(F::CLASS_ELEMENT_IS_BASE, true);
            return;
        }
        if is(s, F::CLASS_ELEMENT_IS_INTERFACE) {
            if interfaces.iter().any(|&x| is(x, F::CLASS_ELEMENT_IS_BASE)) || mixins.iter().any(|&m| is(m, F::CLASS_ELEMENT_IS_BASE)) {
                flags.set(F::CLASS_ELEMENT_IS_FINAL, true);
                return;
            }
            flags.set(F::CLASS_ELEMENT_IS_INTERFACE, true);
            return;
        }
    }
    let base_or_final = |e: EId<InterfaceElement>| is(e, F::CLASS_ELEMENT_IS_BASE) || is(e, F::CLASS_ELEMENT_IS_FINAL);
    if interfaces.iter().any(|&x| base_or_final(x)) || mixins.iter().any(|&m| base_or_final(m)) {
        flags.set(F::CLASS_ELEMENT_IS_BASE, true);
        return;
    }
    if mixins.iter().any(|&m| is(m, F::CLASS_ELEMENT_IS_INTERFACE)) {
        if interfaces.iter().any(|&x| is(x, F::CLASS_ELEMENT_IS_BASE)) {
            flags.set(F::CLASS_ELEMENT_IS_FINAL, true);
            return;
        }
        flags.set(F::CLASS_ELEMENT_IS_INTERFACE, true);
    }
}

/// Dart `buildExtensionTypes`: the type erasure of each extension type, and
/// representation types that depend on themselves.
fn build_extension_types(lk: &Linker<'_>, ctx: &Ctx<'_>) {
    let mut elements: Vec<EId<ExtensionTypeElement>> = Vec::new();
    for b in &lk.builders {
        elements.extend(ctx.get(b.element).extension_types.iter().copied());
    }
    let mut walker = ExtensionTypeWalker {
        ctx,
        store: lk.core.store.id,
        nodes: IndexMap::new(),
        index: 1,
        stack: Vec::new(),
    };
    for e in elements {
        walker.walk(e);
    }
}

struct EtNode {
    index: u32,
    low_link: u32,
    evaluated: bool,
    dependencies: Option<Vec<EId<ExtensionTypeElement>>>,
}

struct ExtensionTypeWalker<'c, 'a> {
    ctx: &'c Ctx<'a>,
    store: StoreId,
    nodes: IndexMap<EId<ExtensionTypeElement>, EtNode>,
    index: u32,
    stack: Vec<EId<ExtensionTypeElement>>,
}

impl ExtensionTypeWalker<'_, '_> {
    fn node(&mut self, e: EId<ExtensionTypeElement>) -> &mut EtNode {
        self.nodes.entry(e).or_insert(EtNode {
            index: 0,
            low_link: 0,
            evaluated: false,
            dependencies: None,
        })
    }

    /// `_Node.computeDependencies`: the extension types of this cycle in
    /// the representation type (without type alias arguments).
    fn dependencies(&mut self, e: EId<ExtensionTypeElement>) -> Vec<EId<ExtensionTypeElement>> {
        if let Some(d) = &self.node(e).dependencies {
            return d.clone();
        }
        let ctx = self.ctx;
        let t = crate::type_bounds::representation_type(ctx, e);
        let mut found = Vec::new();
        collect_extension_types(ctx, t, &mut found);
        let deps: Vec<EId<ExtensionTypeElement>> = found.into_iter().filter(|d| d.store() == self.store).collect();
        self.node(e).dependencies = Some(deps.clone());
        deps
    }

    fn walk(&mut self, e: EId<ExtensionTypeElement>) {
        if self.node(e).evaluated {
            return;
        }
        self.strong_connect(e);
    }

    fn strong_connect(&mut self, e: EId<ExtensionTypeElement>) {
        let mut trivial = false;
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
                trivial = true;
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
                if trivial {
                    self.mark_circular(e);
                } else {
                    let t = crate::type_bounds::representation_type(self.ctx, e);
                    self.evaluate_with_type(e, t);
                }
            } else {
                loop {
                    let other = self.stack.pop().unwrap();
                    self.mark_circular(other);
                    if other == e {
                        break;
                    }
                }
            }
        }
    }

    fn evaluate_with_type(&mut self, e: EId<ExtensionTypeElement>, t: TypeId) {
        let ts = TypeSystem::new(*self.ctx);
        let erasure = ts.extension_type_erasure(t);
        let slot = &self.ctx.get(e).type_erasure;
        if !slot.is_set() {
            slot.set_once(erasure);
        }
        self.node(e).evaluated = true;
    }

    fn mark_circular(&mut self, e: EId<ExtensionTypeElement>) {
        let ctx = self.ctx;
        ctx.get(e).has_representation_self_reference.set(true);
        if let Some(&representation) = ctx.get(e).fields.first() {
            set_variable_type(ctx, representation.upcast(), TypeId::INVALID);
        }
        self.evaluate_with_type(e, TypeId::INVALID);
    }
}

/// `RecursiveTypeVisitor(includeTypeAliasArguments: false)` that collects
/// extension type elements.
fn collect_extension_types(ctx: &Ctx<'_>, t: TypeId, out: &mut Vec<EId<ExtensionTypeElement>>) {
    match *ctx.ty(t) {
        TypeKind::Interface { element, args, .. } => {
            if let Some(e) = element.raw().cast::<ExtensionTypeElement>() {
                out.push(e);
            }
            for &a in ctx.list(args) {
                collect_extension_types(ctx, a, out);
            }
        }
        TypeKind::Function(f) => {
            for &tp in ctx.list(f.type_params) {
                if let Some(b) = ctx.get(tp).bound.get() {
                    collect_extension_types(ctx, b, out);
                }
            }
            for p in ctx.list(f.params) {
                collect_extension_types(ctx, p.ty, out);
            }
            collect_extension_types(ctx, f.ret, out);
        }
        TypeKind::Record { positional, named, .. } => {
            for &p in ctx.list(positional) {
                collect_extension_types(ctx, p, out);
            }
            for n in ctx.list(named) {
                collect_extension_types(ctx, n.ty, out);
            }
        }
        TypeKind::TypeParameter { promoted_bound: Some(b), .. } => collect_extension_types(ctx, b, out),
        _ => {}
    }
}

/// The redirected constructors of `ConstructorInitializerResolver`: Dart
/// resolves the redirection with the `AstResolver` (unit C10). This port
/// resolves the constructor name with the library scope: the type name
/// (also prefixed, also a type alias of a class) and the constructor name,
/// or the constructor of the class for `this.name(...)`. The result is the
/// base element (Dart: the substituted member).
fn resolve_redirected_constructors(lk: &Linker<'_>, ctx: &Ctx<'_>) {
    use dartr_ast::*;
    let scopes = crate::scope::LibraryScopes::build(lk);
    let named = |interface: EId<InterfaceElement>, name: &str| -> Option<ElementId> {
        ctx.interface(interface)
            .constructors
            .iter()
            .find(|&&c| ctx.get(c).name.map(|n| ctx.name_str(n)) == Some(name))
            .map(|c| c.raw())
    };
    for (lib, b) in lk.builders.iter().enumerate() {
        for interface in interfaces_of(ctx, b.element) {
            for &constructor in &ctx.interface(interface).constructors {
                if !first_has(ctx, constructor.raw(), FragmentFlags::CONSTRUCTOR_FRAGMENT_IS_ORIGIN_DECLARATION) {
                    continue;
                }
                let mut fragment = Some(ctx.get(constructor).first_fragment().raw());
                while let Some(f) = fragment {
                    fragment = ctx.fragment_data(f).unwrap().next_fragment;
                    let Some(&(l, unit, node)) = lk.core.fragment_nodes.get(&f) else { continue };
                    let ast = crate::types::unit_ast(lk, l as u32, unit as u32);
                    let Some(c) = ast.cast::<ConstructorDeclaration>(node) else { continue };
                    let c = ast.get(c);
                    if c.factory_keyword.is_some() {
                        let Some(rc) = c.redirected_constructor else { continue };
                        let rc = ast.get(rc);
                        let t = ast.get(rc.type_);
                        let unit_fragment = lk.builders[lib].units[unit].fragment;
                        let type_name = ast.tokens.lexeme(t.name);
                        // `C.name` is parsed as a prefixed type `C.name`; Dart
                        // rewrites it when `C` is not a prefix (AstRewriter).
                        let mut constructor_name: Option<&str> = rc.name.map(|n| ast.tokens.lexeme(ast.get(n).token));
                        let element = match t.import_prefix {
                            Some(p) => {
                                let prefix = ast.tokens.lexeme(ast.get(p).name);
                                match scopes.lookup(unit_fragment, prefix).getter {
                                    Some(crate::scope::ScopeElement::Prefix(_, s)) => {
                                        scopes.prefix_lookup(s, type_name).getter
                                    }
                                    other if constructor_name.is_none() => {
                                        constructor_name = Some(type_name);
                                        other
                                    }
                                    _ => None,
                                }
                            }
                            None => scopes.lookup(unit_fragment, type_name).getter,
                        };
                        let element = element.and_then(|e| e.element());
                        let interface_element = element.and_then(|e| {
                            if let Some(i) = e.cast::<InterfaceElement>() {
                                Some(i)
                            } else if let Some(a) = e.cast::<TypeAliasElement>() {
                                let aliased = ctx.get(a).aliased_type.get()?;
                                interface_element_of(ctx, aliased)
                            } else {
                                None
                            }
                        });
                        let name = constructor_name.unwrap_or("new");
                        let target = interface_element.and_then(|i| named(i, name));
                        ctx.get(constructor).redirected_constructor.set(target.map(ElemRef::Base));
                    } else {
                        for &init in ast.list(c.initializers) {
                            if let Some(r) = ast.cast::<RedirectingConstructorInvocation>(init.raw()) {
                                let name = ast
                                    .get(r)
                                    .constructor_name
                                    .map(|n| ast.tokens.lexeme(ast.get(n).token))
                                    .unwrap_or("new");
                                let target = named(interface, name);
                                ctx.get(constructor).redirected_constructor.set(target.map(ElemRef::Base));
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Dart `SuperFormalParameterElementImpl.superConstructorParameter` (the
/// base element).
pub fn super_constructor_parameter(ctx: &Ctx<'_>, p: EId<FormalParameterElement>) -> Option<EId<FormalParameterElement>> {
    let constructor = ctx.element_data(p.raw())?.enclosing?.cast::<ConstructorElement>()?;
    let ElemRef::Base(super_constructor) = ctx.get(constructor).super_constructor.get()? else {
        return None;
    };
    let super_params = &ctx.get(EId::<ConstructorElement>::from_raw(super_constructor)).formal_params;
    let pe = ctx.get(p);
    if pe.kind.is_named() {
        super_params
            .iter()
            .copied()
            .find(|&s| ctx.get(s).kind.is_named() && ctx.get(s).name == pe.name)
    } else {
        let index = ctx
            .get(constructor)
            .formal_params
            .iter()
            .copied()
            .filter(|x| x.raw().tag() == Tag::SuperFormalParameter && ctx.get(*x).kind.is_positional())
            .position(|x| x == p)?;
        super_params
            .iter()
            .copied()
            .filter(|&s| ctx.get(s).kind.is_positional())
            .nth(index)
    }
}

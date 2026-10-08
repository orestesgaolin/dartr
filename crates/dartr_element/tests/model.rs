//! Tests of the element model: id packing, typed id casts, the interner
//! (dedup, overlay, concurrency), set-once slots, and a small hand-built
//! world read through `Ctx`.

use std::sync::Arc;

use dartr_element::diagnostics::{element_ref, library_fragment_of, type_elements};
use dartr_element::*;

// ---------------------------------------------------------------- ids

#[test]
fn element_id_packs_store_tag_and_index() {
    let cases = [
        (StoreId::SYNTHETIC, Tag::Class, 0),
        (StoreId::cycle(1), Tag::FieldFormalParameter, 7),
        (StoreId::cycle(StoreId::MAX_INDEX), Tag::Never, u32::MAX),
        (StoreId::local(0), Tag::LocalVariable, 123_456),
        (StoreId::local(StoreId::MAX_INDEX), Tag::Label, 1),
    ];
    for (store, tag, index) in cases {
        let id = ElementId::new(store, tag, index);
        assert_eq!((id.store(), id.tag(), id.index()), (store, tag, index));
        let f = FragmentId::new(store, tag, index);
        assert_eq!((f.store(), f.tag(), f.index()), (store, tag, index));
    }
    // Every tag survives packing next to the largest store and index.
    for &tag in Tag::ALL {
        let id = ElementId::new(StoreId::local(StoreId::MAX_INDEX), tag, u32::MAX);
        assert_eq!(id.tag(), tag);
        assert!(id.store().is_local());
    }
    assert_eq!(StoreId::SYNTHETIC.kind(), StoreKind::Synthetic);
    assert_eq!(StoreId::cycle(5).kind(), StoreKind::Cycle);
    assert_eq!(StoreId::local(5).kind(), StoreKind::Local);
    assert_ne!(StoreId::cycle(5), StoreId::local(5));
    // `Option<ElementId>` has the size of the id (the tag is never 0).
    assert_eq!(std::mem::size_of::<Option<ElementId>>(), 8);
}

#[test]
#[should_panic(expected = "cycle store index")]
fn cycle_store_zero_is_reserved_for_the_synthetic_store() {
    StoreId::cycle(0);
}

#[test]
fn typed_ids_cast_by_tag_like_dart_is() {
    let store = StoreId::cycle(3);
    let field_formal = ElementId::new(store, Tag::FieldFormalParameter, 4);

    // Dart: `p is FormalParameterElement`, `p is VariableElement`, ...
    let as_param: EId<FormalParameterElement> = field_formal.cast().expect("a formal parameter");
    assert!(field_formal.is::<VariableElement>());
    assert!(field_formal.is::<LocalElement>());
    assert!(field_formal.is::<FieldFormalParameterElement>());
    assert!(!field_formal.is::<SuperFormalParameterElement>());
    assert!(!field_formal.is::<FieldElement>());
    assert!(field_formal.cast::<PropertyInducingElement>().is_none());

    // Narrowing from a category, then widening back (compile-time checked).
    let narrow: EId<FieldFormalParameterElement> = as_param.cast().unwrap();
    let wide: EId<VariableElement> = narrow.upcast();
    assert_eq!(wide.raw(), field_formal);
    assert_eq!(narrow.upcast::<FormalParameterElement>(), as_param);

    let mixin = ElementId::new(store, Tag::Mixin, 0);
    assert!(mixin.is::<InterfaceElement>());
    assert!(mixin.is::<InstanceElement>());
    assert!(mixin.is::<TypeDefiningElement>());
    assert!(!mixin.is::<ExecutableElement>());
    let extension = ElementId::new(store, Tag::Extension, 0);
    assert!(extension.is::<InstanceElement>());
    assert!(!extension.is::<InterfaceElement>());

    let getter = ElementId::new(store, Tag::Getter, 1);
    assert!(getter.is::<PropertyAccessorElement>());
    assert!(getter.is::<ExecutableElement>());
    assert!(!getter.is::<MethodElement>());
    assert_eq!(getter.kind(), ElementKind::Getter);
    assert_eq!(getter.kind().display_name(), "getter");
    assert_eq!(
        ElementId::new(store, Tag::LocalFunction, 0).kind().name(),
        "FUNCTION"
    );

    let join = FragmentId::new(StoreId::local(1), Tag::JoinPatternVariable, 2);
    assert!(join.is::<PatternVariableFragment>());
    assert!(join.is::<LocalVariableFragment>());
    assert!(!join.is::<BindPatternVariableFragment>());
}

// ---------------------------------------------------------------- slots

#[test]
#[should_panic(expected = "OnceSlot set twice")]
fn once_slot_panics_on_second_set() {
    let slot = OnceSlot::new();
    slot.set_once(TypeId::DYNAMIC);
    assert_eq!(*slot.get(), TypeId::DYNAMIC);
    slot.set_once(TypeId::VOID);
}

#[test]
#[should_panic(expected = "read before it was set")]
fn once_slot_panics_on_read_before_set() {
    let slot: OnceSlot<TypeId> = OnceSlot::new();
    slot.get();
}

#[test]
fn var_slot_and_flag_cells_change_through_shared_refs() {
    let slot = VarSlot::with(TypeId::INVALID);
    let r = &slot;
    r.set(Some(TypeId::DYNAMIC));
    assert_eq!(slot.get(), Some(TypeId::DYNAMIC));
    r.set(None);
    assert_eq!(slot.get(), None);

    let flags = FragmentFlagCell::default();
    flags.set(FragmentFlags::CLASS_FRAGMENT_IS_ABSTRACT, true);
    flags.set(FragmentFlags::VARIABLE_FRAGMENT_IS_LATE, true);
    flags.set(FragmentFlags::CLASS_FRAGMENT_IS_ABSTRACT, false);
    assert!(!flags.has(FragmentFlags::CLASS_FRAGMENT_IS_ABSTRACT));
    assert!(flags.has(FragmentFlags::VARIABLE_FRAGMENT_IS_LATE));
    assert_eq!(format!("{:?}", flags), "{\"variableFragment_isLate\"}");
}

// ---------------------------------------------------------------- names

#[test]
fn names_are_interned_and_well_known_names_are_constants() {
    let names = NamePool::new();
    assert_eq!(names.intern("new"), Name::NEW);
    assert_eq!(names.intern("[]="), Name::INDEX_SET);
    assert_eq!(names.get(Name::GT_GT_GT), ">>>");
    let foo = names.intern("foo");
    assert_eq!(names.intern("foo"), foo);
    assert_ne!(names.intern("Foo"), foo);
    assert_eq!(names.lookup("foo"), Some(foo));
    assert_eq!(names.lookup("bar"), None);
}

// ---------------------------------------------------------------- interner

fn interface(element: EId<InterfaceElement>, args: TypeList, nullability: Nullability) -> TypeKind {
    TypeKind::Interface {
        element,
        args,
        nullability,
        alias: None,
    }
}

#[test]
fn interner_deduplicates_structurally_equal_types() {
    let interner = Interner::new();
    // The fixed types have their constant ids.
    assert_eq!(interner.intern(TypeKind::Dynamic), TypeId::DYNAMIC);
    assert_eq!(
        interner.intern(TypeKind::Never(Nullability::Question)),
        TypeId::NEVER_QUESTION
    );

    let list = EId::<InterfaceElement>::from_raw(ElementId::new(StoreId::cycle(1), Tag::Class, 0));
    let int = EId::<InterfaceElement>::from_raw(ElementId::new(StoreId::cycle(1), Tag::Class, 1));
    let int_t = interner.intern(interface(int, TypeList::EMPTY, Nullability::None));
    let int_q = interner.intern(interface(int, TypeList::EMPTY, Nullability::Question));
    assert_ne!(int_t, int_q);

    let args1 = interner.intern_list(&[int_t]);
    let args2 = interner.intern_list(&[int_t]);
    assert_eq!(args1, args2);
    assert_eq!(interner.list(args1), &[int_t]);
    assert!(interner.intern_list::<TypeId>(&[]).is_empty());

    let a = interner.intern(interface(list, args1, Nullability::None));
    let b = interner.intern(interface(list, args2, Nullability::None));
    assert_eq!(a, b, "List<int> is interned once");
    assert_eq!(*interner.get(a), interface(list, args1, Nullability::None));

    // An alias makes a different identity (design §1.4: TypeId is exact
    // structural identity, not Dart `==`).
    let alias_el = EId::from_raw(ElementId::new(StoreId::cycle(1), Tag::TypeAlias, 0));
    let alias = interner.intern_alias(AliasRef {
        element: alias_el,
        args: TypeList::EMPTY,
        nullability: Nullability::None,
    });
    let aliased = interner.intern(TypeKind::Interface {
        element: list,
        args: args1,
        nullability: Nullability::None,
        alias: Some(alias),
    });
    assert_ne!(aliased, a);

    // Substitutions are canonical regardless of the input order.
    let t = EId::from_raw(ElementId::new(StoreId::cycle(1), Tag::TypeParameter, 0));
    let u = EId::from_raw(ElementId::new(StoreId::cycle(1), Tag::TypeParameter, 1));
    let s1 = interner.intern_subst(&mut [(t, int_t), (u, a)]);
    let s2 = interner.intern_subst(&mut [(u, a), (t, int_t)]);
    assert_eq!(s1, s2);
    let s3 = interner.intern_subst(&mut [(u, int_t), (t, a)]);
    assert_ne!(s1, s3);
}

#[test]
fn interner_deduplicates_across_threads() {
    let interner = Arc::new(Interner::new());
    let elements: Vec<EId<InterfaceElement>> = (0..200)
        .map(|i| EId::from_raw(ElementId::new(StoreId::cycle(2), Tag::Class, i)))
        .collect();
    let results: Vec<Vec<TypeId>> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..8)
            .map(|k| {
                let interner = &interner;
                let elements = &elements;
                s.spawn(move || {
                    // Each thread interns the same types in a different order.
                    let mut ids = vec![None; elements.len()];
                    for j in 0..elements.len() {
                        let i = (j * 7 + k * 31) % elements.len();
                        let arg = interner.intern(interface(
                            elements[(i + 1) % 200],
                            TypeList::EMPTY,
                            Nullability::None,
                        ));
                        let args = interner.intern_list(&[arg, TypeId::DYNAMIC]);
                        ids[i] =
                            Some(interner.intern(interface(elements[i], args, Nullability::None)));
                    }
                    ids.into_iter().map(Option::unwrap).collect()
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for r in &results[1..] {
        assert_eq!(r, &results[0]);
    }
    // 6 fixed + 200 bare + 200 with args.
    assert_eq!(interner.type_count(), 406);
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "a global type must not contain a local id")]
fn global_interner_rejects_local_ids() {
    let interner = Interner::new();
    let local_param = EId::from_raw(ElementId::new(StoreId::local(1), Tag::TypeParameter, 0));
    interner.intern(TypeKind::TypeParameter {
        param: local_param,
        nullability: Nullability::None,
        promoted_bound: None,
        alias: None,
    });
}

#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "a global type must not contain a local id")]
fn global_interner_rejects_local_type_arguments() {
    let interner = Interner::new();
    let overlay = TypeOverlay::new();
    let local_param = EId::from_raw(ElementId::new(StoreId::local(1), Tag::TypeParameter, 0));
    let local_t = overlay.intern(TypeKind::TypeParameter {
        param: local_param,
        nullability: Nullability::None,
        promoted_bound: None,
        alias: None,
    });
    interner.intern_list(&[TypeId::DYNAMIC, local_t]);
}

// ---------------------------------------------------------------- a small world

/// The world of the tests:
///
/// ```dart
/// // dart:core (store A)
/// abstract class Comparable<T> {}
/// // package:p/a.dart (store B)
/// class C<T extends Comparable<T>> {}
/// ```
struct World {
    world: WorldSnapshot,
    tp: TypeProvider,
    features: FeatureSet,
    comparable: EId<ClassElement>,
    class_c: EId<ClassElement>,
    t: EId<TypeParameterElement>,
}

fn add_library(
    store: &ElementStore,
    names: &NamePool,
    uri: &str,
    path: &str,
) -> (EId<LibraryElement>, FId<LibraryFragment>) {
    let unit = FId::<LibraryFragment>::from_raw(FragmentId::new(store.id, Tag::Library, 0));
    let library: EId<LibraryElement> = store.add(LibraryElement {
        element: ElementData::new(Some(names.intern("")), unit.raw()),
        metadata: Metadata::default(),
        documentation_comment: None,
        language_version: LibraryLanguageVersion {
            package: Version {
                major: 3,
                minor: 13,
            },
            override_: None,
        },
        feature_set: FeatureSet::default(),
        entry_point: OnceSlot::new(),
        load_library_function: OnceSlot::new(),
        name_offset: -1,
        name_length: 0,
        classes: Vec::new(),
        enums: Vec::new(),
        extensions: Vec::new(),
        extension_types: Vec::new(),
        getters: Vec::new(),
        setters: Vec::new(),
        mixins: Vec::new(),
        top_level_functions: Vec::new(),
        top_level_variables: Vec::new(),
        type_aliases: Vec::new(),
        export_namespace: OnceSlot::new(),
        public_namespace: OnceSlot::new(),
        field_name_non_promotability_info: OnceSlot::new(),
    });
    let source = SourceRef {
        path: Arc::from(path),
        uri: Arc::from(uri),
    };
    let added = store.add_fragment::<LibraryFragment>(LibraryFragment::new(
        FragmentData::new(None, None),
        source,
        library,
    ));
    assert_eq!(added, unit);
    store.fragment(unit).element.set_once(library.raw());
    (library, unit)
}

/// Builds `class <name><type params>` in [store] (builder phase: `&mut`).
fn add_class(
    store: &mut ElementStore,
    names: &NamePool,
    library: EId<LibraryElement>,
    unit: FId<LibraryFragment>,
    name: &str,
    name_offset: u32,
    type_params: &[(&str, u32)],
) -> (EId<ClassElement>, Vec<EId<TypeParameterElement>>) {
    let mut fragment = FragmentData::new(Some(names.intern(name)), Some(name_offset));
    fragment.enclosing_fragment = Some(unit.raw());
    let class_fragment = store.add_fragment::<ClassFragment>(ClassFragment {
        interface: InterfaceFragmentData::new(fragment),
    });
    let mut element = ElementData::new(Some(names.intern(name)), class_fragment.raw());
    element.library = Some(library);
    element.enclosing = Some(library.raw());
    let class: EId<ClassElement> = store.add(ClassElement {
        interface: InterfaceElementData::new(element),
    });
    store.fragment(class_fragment).element.set_once(class.raw());

    let mut params = Vec::new();
    for &(tp_name, offset) in type_params {
        let mut f = FragmentData::new(Some(names.intern(tp_name)), Some(offset));
        f.enclosing_fragment = Some(class_fragment.raw());
        let tp_fragment =
            store.add_fragment::<TypeParameterFragment>(TypeParameterFragment { fragment: f });
        let mut e = ElementData::new(Some(names.intern(tp_name)), tp_fragment.raw());
        e.library = Some(library);
        e.enclosing = Some(class.raw());
        let tp: EId<TypeParameterElement> = store.add(TypeParameterElement::new(e));
        store.fragment(tp_fragment).element.set_once(tp.raw());
        store
            .fragment_mut(class_fragment)
            .type_params
            .push(tp_fragment);
        params.push(tp);
    }
    store.get_mut(class).type_params = params.clone();
    store.get_mut(library).classes.push(class);
    store.fragment_mut(unit).classes.push(class_fragment);
    (class, params)
}

fn build_world() -> World {
    let generation = Arc::new(Generation::new(0));
    let names = &generation.names;
    let tp = TypeProvider::default();
    let features = FeatureSet::default();

    // Cycle A: dart:core, linked and frozen first.
    let mut core = generation.new_cycle_store();
    let (core_lib, core_unit) = add_library(&core, names, "dart:core", "/sdk/core.dart");
    let (comparable, comparable_params) = add_class(
        &mut core,
        names,
        core_lib,
        core_unit,
        "Comparable",
        15,
        &[("T", 26)],
    );
    let core = Arc::new(core);
    let world = WorldSnapshot::new(generation.clone())
        .with_store(core.clone(), [(Arc::from("dart:core"), core_lib)]);
    // Phase "resolve types" of cycle A: Comparable<T> has no bound.
    {
        let ctx = Ctx {
            world: &world,
            current: None,
            local: None,
            tp: &tp,
            features: &features,
            req: &NoopSink,
        };
        let object_bound = ctx.get(comparable_params[0]);
        assert_eq!(object_bound.bound.get(), None);
    }

    // Cycle B: package:p/a.dart. Builder phase with `&mut`, then the
    // "resolve types" phase writes the bound through a shared borrow.
    let mut store = generation.new_cycle_store();
    let (lib, unit) = add_library(&store, names, "package:p/a.dart", "/p/lib/a.dart");
    let (class_c, params) = add_class(&mut store, names, lib, unit, "C", 6, &[("T", 8)]);
    let t = params[0];
    {
        let ctx = Ctx {
            world: &world,
            current: Some(&store),
            local: None,
            tp: &tp,
            features: &features,
            req: &NoopSink,
        };
        let t_type = ctx.intern(TypeKind::TypeParameter {
            param: t,
            nullability: Nullability::None,
            promoted_bound: None,
            alias: None,
        });
        let args = ctx.intern_list(&[t_type]);
        let bound = ctx.intern(TypeKind::Interface {
            element: comparable.upcast(),
            args,
            nullability: Nullability::None,
            alias: None,
        });
        ctx.get(t).bound.set(Some(bound));
    }
    let world = world.with_store(Arc::new(store), [(Arc::from("package:p/a.dart"), lib)]);
    World {
        world,
        tp,
        features,
        comparable,
        class_c,
        t,
    }
}

impl World {
    fn ctx(&self) -> Ctx<'_> {
        Ctx {
            world: &self.world,
            current: None,
            local: None,
            tp: &self.tp,
            features: &self.features,
            req: &NoopSink,
        }
    }
}

#[test]
fn class_with_f_bounded_type_parameter_reads_through_ctx() {
    let w = build_world();
    let ctx = w.ctx();

    let lib = ctx.library_by_uri("package:p/a.dart").unwrap();
    assert_eq!(ctx.get(lib).classes, vec![w.class_c]);
    let class = ctx.get(w.class_c);
    assert_eq!(ctx.name_str(class.name.unwrap()), "C");
    assert_eq!(class.library, Some(lib));
    assert_eq!(class.type_params, vec![w.t]);

    // Same data through the category accessors.
    let iface = ctx.interface(w.class_c.upcast());
    assert_eq!(iface.type_params, vec![w.t]);
    assert!(matches!(ctx.any(w.class_c.raw()), AnyElement::Class(_)));

    // T extends Comparable<T>: an id cycle bound -> type -> param.
    let bound = ctx.get(w.t).bound.get().expect("bound");
    let TypeKind::Interface {
        element,
        args,
        nullability,
        alias,
    } = *ctx.ty(bound)
    else {
        panic!("bound is not an interface type");
    };
    assert_eq!(element, w.comparable.upcast());
    assert_eq!(
        ctx.name_str(ctx.interface(element).name.unwrap()),
        "Comparable"
    );
    assert_eq!((nullability, alias), (Nullability::None, None));
    let [arg] = ctx.list(args) else {
        panic!("one type argument")
    };
    let TypeKind::TypeParameter { param, .. } = *ctx.ty(*arg) else {
        panic!("argument is not a type parameter type");
    };
    assert_eq!(param, w.t);

    // Fragments link back to their elements and units.
    let fragment = ctx.fragment(class.first_fragment());
    assert_eq!(*fragment.element.get(), w.class_c.raw());
    assert_eq!(fragment.type_params.len(), 1);
    let unit = library_fragment_of(&ctx, class.first_fragment().raw()).unwrap();
    assert_eq!(&*ctx.fragment(unit).source.uri, "package:p/a.dart");

    // The bound type is global: interning it again from a cycle-free
    // context gives the same id.
    let again = ctx.intern(*ctx.ty(bound));
    assert_eq!(again, bound);
}

#[test]
fn diagnostic_arguments_locate_the_element() {
    let w = build_world();
    let ctx = w.ctx();
    let r = element_ref(&ctx, w.class_c.raw());
    assert_eq!(r.name.as_deref(), Some("C"));
    assert_eq!(r.source_path, "/p/lib/a.dart");
    assert_eq!((r.offset, r.length), (6, 1));
    assert!(!r.is_extension);

    // `_TypeToConvert.allElements` of `Comparable<T>` is [Comparable] (type
    // parameters are not interface elements).
    let bound = ctx.get(w.t).bound.expect();
    assert_eq!(type_elements(&ctx, bound), vec![w.comparable.raw()]);
    let comparable = element_ref(&ctx, w.comparable.raw());
    assert_eq!(comparable.source_path, "/sdk/core.dart");
    assert_eq!((comparable.offset, comparable.length), (15, 10));
}

#[test]
fn local_overlay_holds_types_with_local_elements_only() {
    let w = build_world();
    let local = w.world.generation.new_local_arena();
    let ctx = Ctx {
        local: Some(&local),
        ..w.ctx()
    };
    let global_before = w.world.generation.interner.type_count();

    // A local type parameter `S extends Comparable<T>` of a local function.
    let s_fragment = local
        .store
        .add_fragment::<TypeParameterFragment>(TypeParameterFragment {
            fragment: FragmentData::new(Some(ctx.name("S")), Some(40)),
        });
    let s: EId<TypeParameterElement> = local.store.add(TypeParameterElement::new(
        ElementData::new(Some(ctx.name("S")), s_fragment.raw()),
    ));
    assert!(s.store().is_local());
    let s_type = ctx.intern(TypeKind::TypeParameter {
        param: s,
        nullability: Nullability::Question,
        promoted_bound: None,
        alias: None,
    });
    assert!(s_type.is_local());

    // A type that mentions S is local; reading it works through ctx.
    let args = ctx.intern_list(&[s_type]);
    assert!(args.is_local());
    let comparable_s = ctx.intern(TypeKind::Interface {
        element: w.comparable.upcast(),
        args,
        nullability: Nullability::None,
        alias: None,
    });
    assert!(comparable_s.is_local());
    assert_eq!(ctx.list(args), &[s_type]);
    assert_eq!(ctx.get(s).name, Some(ctx.name("S")));

    // A type without local ids goes to the global interner, so its id is the
    // same as from a global context.
    let comparable_dynamic_args = ctx.intern_list(&[TypeId::DYNAMIC]);
    let comparable_dynamic = ctx.intern(TypeKind::Interface {
        element: w.comparable.upcast(),
        args: comparable_dynamic_args,
        nullability: Nullability::None,
        alias: None,
    });
    assert!(!comparable_dynamic.is_local());
    let g = ctx.global();
    let args_g = g.intern_list(&[TypeId::DYNAMIC]);
    assert_eq!(
        g.intern(TypeKind::Interface {
            element: w.comparable.upcast(),
            args: args_g,
            nullability: Nullability::None,
            alias: None
        }),
        comparable_dynamic
    );
    // Local types did not grow the global interner (only Comparable<dynamic>).
    assert_eq!(w.world.generation.interner.type_count(), global_before + 1);
}

#[test]
#[should_panic(expected = "needs a context with a LocalArena")]
fn reading_a_local_type_without_the_arena_panics() {
    let w = build_world();
    let local = w.world.generation.new_local_arena();
    let with_local = Ctx {
        local: Some(&local),
        ..w.ctx()
    };
    let s: EId<TypeParameterElement> = local.store.add(TypeParameterElement::new(
        ElementData::new(None, FragmentId::new(local.store.id, Tag::TypeParameter, 0)),
    ));
    let s_type = with_local.intern(TypeKind::TypeParameter {
        param: s,
        nullability: Nullability::None,
        promoted_bound: None,
        alias: None,
    });
    // A shared cache runs under ctx.global() and must never see S.
    with_local.global().ty(s_type);
}

#[test]
fn resolution_tables_map_nodes_to_semantic_data() {
    let w = build_world();
    let mut tables = ResolutionTables::new();
    let node = dartr_ast::NodeId::from_index(41);
    let bound = w.ctx().get(w.t).bound.expect();
    tables.static_type.insert(node, bound);
    tables.element.insert(node, ElemRef::Base(w.class_c.raw()));
    assert_eq!(tables.static_type.get(node), Some(&bound));
    assert_eq!(
        tables.element.get(node),
        Some(&ElemRef::Base(w.class_c.raw()))
    );
    assert_eq!(
        tables.static_type.get(dartr_ast::NodeId::from_index(40)),
        None
    );
    assert_eq!(tables.invoke_type.get(node), None);
}

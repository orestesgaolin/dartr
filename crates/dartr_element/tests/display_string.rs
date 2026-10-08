//! Tests of the display strings of types and elements (unit A1, port of
//! `display_string_builder.dart`). Each test builds a small world by hand
//! (one unfrozen cycle store read through `Ctx`) and compares the text with
//! what the Dart analyzer writes for the same types and elements.

use std::sync::Arc;

use dartr_element::diagnostics::{element_display_string, type_display_string};
use dartr_element::*;

// ---------------------------------------------------------------- world

struct W {
    world: WorldSnapshot,
    store: ElementStore,
    tp: TypeProvider,
    features: FeatureSet,
    core: EId<LibraryElement>,
    lib: EId<LibraryElement>,
    unit: FId<LibraryFragment>,
}

impl W {
    fn new() -> W {
        let generation = Arc::new(Generation::new(0));
        let store = generation.new_cycle_store();
        let world = WorldSnapshot::new(generation);
        let mut w = W {
            world,
            store,
            tp: TypeProvider::default(),
            features: FeatureSet::default(),
            core: EId::from_raw(ElementId::DYNAMIC),
            lib: EId::from_raw(ElementId::DYNAMIC),
            unit: FId::from_raw(FragmentId::new(StoreId::SYNTHETIC, Tag::Library, 0)),
        };
        let (core, _) = w.add_library("dart.core", "dart:core");
        let (lib, unit) = w.add_library("", "package:p/a.dart");
        w.core = core;
        w.lib = lib;
        w.unit = unit;
        w
    }

    fn ctx(&self) -> Ctx<'_> {
        Ctx {
            world: &self.world,
            current: Some(&self.store),
            local: None,
            tp: &self.tp,
            features: &self.features,
            req: &NoopSink,
        }
    }

    fn name(&self, text: &str) -> Name {
        self.world.generation.names.intern(text)
    }

    fn add_library(&self, name: &str, uri: &str) -> (EId<LibraryElement>, FId<LibraryFragment>) {
        let index = self.store.fragments.units.len() as u32;
        let unit =
            FId::<LibraryFragment>::from_raw(FragmentId::new(self.store.id, Tag::Library, index));
        let library: EId<LibraryElement> = self.store.add(LibraryElement {
            element: ElementData::new(Some(self.name(name)), unit.raw()),
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
            path: Arc::from(format!("/{uri}")),
            uri: Arc::from(uri),
        };
        let added = self
            .store
            .add_fragment::<LibraryFragment>(LibraryFragment::new(
                FragmentData::new(None, None),
                source,
                library,
            ));
        assert_eq!(added, unit);
        self.store.fragment(unit).element.set_once(library.raw());
        (library, unit)
    }

    /// Element data whose first fragment is never read by the display code.
    fn data(&self, name: Option<&str>, tag: Tag) -> ElementData {
        let mut d = ElementData::new(
            name.map(|n| self.name(n)),
            FragmentId::new(self.store.id, tag, 0),
        );
        d.library = Some(self.lib);
        d
    }

    // ---- elements ----

    fn type_param(&self, name: Option<&str>) -> EId<TypeParameterElement> {
        self.store.add(TypeParameterElement::new(
            self.data(name, Tag::TypeParameter),
        ))
    }

    fn type_param_with(
        &self,
        name: &str,
        variance: Option<Variance>,
        bound: Option<TypeId>,
    ) -> EId<TypeParameterElement> {
        let mut e = TypeParameterElement::new(self.data(Some(name), Tag::TypeParameter));
        e.variance = variance;
        e.bound.set(bound);
        self.store.add(e)
    }

    fn class_in(
        &self,
        library: EId<LibraryElement>,
        name: &str,
        type_params: Vec<EId<TypeParameterElement>>,
    ) -> EId<ClassElement> {
        let fragment = self.store.add_fragment::<ClassFragment>(ClassFragment {
            interface: InterfaceFragmentData::new(FragmentData::new(Some(self.name(name)), None)),
        });
        let mut data = ElementData::new(Some(self.name(name)), fragment.raw());
        data.library = Some(library);
        data.enclosing = Some(library.raw());
        let mut iface = InterfaceElementData::new(data);
        iface.instance.type_params = type_params;
        let class: EId<ClassElement> = self.store.add(ClassElement { interface: iface });
        self.store.fragment(fragment).element.set_once(class.raw());
        class
    }

    fn class(&self, name: &str, type_params: Vec<EId<TypeParameterElement>>) -> EId<ClassElement> {
        self.class_in(self.lib, name, type_params)
    }

    fn param(
        &self,
        name: Option<&str>,
        kind: ParameterKind,
        ty: Option<TypeId>,
    ) -> EId<FormalParameterElement> {
        let e = FormalParameterElement {
            variable: VariableElementData::new(self.data(name, Tag::FormalParameter)),
            kind,
            type_: VarSlot::new(),
            base_formal_parameter: None,
            field: VarSlot::new(),
        };
        e.type_.set(ty);
        self.store.add(e)
    }

    fn executable(
        &self,
        name: Option<&str>,
        tag: Tag,
        type_params: Vec<EId<TypeParameterElement>>,
        params: Vec<EId<FormalParameterElement>>,
        return_type: Option<TypeId>,
    ) -> ExecutableElementData {
        let mut e = ExecutableElementData::new(self.data(name, tag));
        e.type_params = type_params;
        e.formal_params = params;
        e.return_type.set(return_type);
        e
    }

    fn method(
        &self,
        name: &str,
        type_params: Vec<EId<TypeParameterElement>>,
        params: Vec<EId<FormalParameterElement>>,
        return_type: TypeId,
    ) -> EId<MethodElement> {
        self.store.add(MethodElement {
            executable: self.executable(
                Some(name),
                Tag::Method,
                type_params,
                params,
                Some(return_type),
            ),
            is_operator_equal_with_parameter_type_from_object: BoolSlot::new(false),
            type_inference_error: OnceSlot::new(),
        })
    }

    // ---- types ----

    fn iface<T>(&self, element: EId<T>, args: &[TypeId], n: Nullability) -> TypeId
    where
        T: SubtypeOf<InterfaceElement> + ?Sized,
    {
        let ctx = self.ctx();
        let args = ctx.intern_list(args);
        ctx.intern(TypeKind::Interface {
            element: element.upcast(),
            args,
            nullability: n,
            alias: None,
        })
    }

    fn tpt(&self, param: EId<TypeParameterElement>, n: Nullability) -> TypeId {
        self.ctx().intern(TypeKind::TypeParameter {
            param,
            nullability: n,
            promoted_bound: None,
            alias: None,
        })
    }

    fn func(
        &self,
        type_params: &[EId<TypeParameterElement>],
        params: &[(ParameterKind, Option<&str>, TypeId)],
        ret: TypeId,
        n: Nullability,
    ) -> TypeId {
        let ctx = self.ctx();
        let fn_params: Vec<FnParam> = params
            .iter()
            .map(|&(kind, name, ty)| FnParam {
                name: name.map(|n| self.name(n)),
                kind,
                ty,
                covariant: false,
                element: None,
            })
            .collect();
        let required_positional = params
            .iter()
            .filter(|p| p.0 == ParameterKind::Required)
            .count() as u16;
        ctx.intern(TypeKind::Function(FunctionTypeData {
            type_params: ctx.intern_list(type_params),
            params: ctx.intern_list(&fn_params),
            required_positional,
            ret,
            nullability: n,
            alias: None,
        }))
    }

    fn record(&self, positional: &[TypeId], named: &[(&str, TypeId)], n: Nullability) -> TypeId {
        let ctx = self.ctx();
        let mut named: Vec<NamedType> = named
            .iter()
            .map(|&(name, ty)| NamedType {
                name: self.name(name),
                ty,
            })
            .collect();
        named.sort_by(|a, b| ctx.name_str(a.name).cmp(ctx.name_str(b.name)));
        ctx.intern(TypeKind::Record {
            positional: ctx.intern_list(positional),
            named: ctx.intern_list(&named),
            nullability: n,
            alias: None,
        })
    }

    fn ty(&self, ty: TypeId) -> String {
        type_display_string_with(&self.ctx(), ty, DisplayOptions::default())
    }

    fn el(&self, element: ElementId) -> String {
        element_display_string(&self.ctx(), element)
    }
}

use Nullability::{None as N, Question as Q, Star};
use ParameterKind::{Named, NamedRequired, Positional, Required};

/// `int`, `String`, `num`, `bool`, `List<E>`, `Map<K, V>`, `Object` (core).
struct Core {
    int: EId<ClassElement>,
    string: EId<ClassElement>,
    num: EId<ClassElement>,
    boolean: EId<ClassElement>,
    list: EId<ClassElement>,
    map: EId<ClassElement>,
    object: EId<ClassElement>,
}

fn core(w: &W) -> Core {
    let e = w.type_param(Some("E"));
    let k = w.type_param(Some("K"));
    let v = w.type_param(Some("V"));
    Core {
        int: w.class_in(w.core, "int", vec![]),
        string: w.class_in(w.core, "String", vec![]),
        num: w.class_in(w.core, "num", vec![]),
        boolean: w.class_in(w.core, "bool", vec![]),
        list: w.class_in(w.core, "List", vec![e]),
        map: w.class_in(w.core, "Map", vec![k, v]),
        object: w.class_in(w.core, "Object", vec![]),
    }
}

// ---------------------------------------------------------------- types

#[test]
fn fixed_types() {
    let w = W::new();
    assert_eq!(w.ty(TypeId::DYNAMIC), "dynamic");
    assert_eq!(w.ty(TypeId::VOID), "void");
    assert_eq!(w.ty(TypeId::INVALID), "InvalidType");
    assert_eq!(w.ty(TypeId::UNKNOWN), "_");
    assert_eq!(w.ty(TypeId::NEVER), "Never");
    assert_eq!(w.ty(TypeId::NEVER_QUESTION), "Never?");
    let never_star = w.ctx().intern(TypeKind::Never(Star));
    assert_eq!(w.ty(never_star), "Never*");
}

#[test]
fn interface_types_with_arguments_and_suffixes() {
    let w = W::new();
    let c = core(&w);
    let int = w.iface(c.int, &[], N);
    let int_q = w.iface(c.int, &[], Q);
    let string = w.iface(c.string, &[], N);
    assert_eq!(w.ty(int), "int");
    assert_eq!(w.ty(int_q), "int?");
    assert_eq!(w.ty(w.iface(c.int, &[], Star)), "int*");
    let map = w.iface(c.map, &[string, int_q], Q);
    assert_eq!(w.ty(map), "Map<String, int?>?");
    let list_of_map = w.iface(c.list, &[map], N);
    assert_eq!(w.ty(list_of_map), "List<Map<String, int?>?>");
}

#[test]
fn record_types() {
    let w = W::new();
    let c = core(&w);
    let int = w.iface(c.int, &[], N);
    let string = w.iface(c.string, &[], N);
    let boolean = w.iface(c.boolean, &[], N);
    assert_eq!(w.ty(w.record(&[], &[], N)), "()");
    assert_eq!(w.ty(w.record(&[int], &[], N)), "(int,)");
    assert_eq!(w.ty(w.record(&[int], &[], Q)), "(int,)?");
    assert_eq!(w.ty(w.record(&[int, string], &[], N)), "(int, String)");
    assert_eq!(
        w.ty(w.record(&[], &[("b", string), ("a", int)], N)),
        "({int a, String b})"
    );
    assert_eq!(
        w.ty(w.record(&[int, string], &[("flag", boolean)], Q)),
        "(int, String, {bool flag})?"
    );
    // One positional field and a named field: no trailing comma.
    assert_eq!(w.ty(w.record(&[int], &[("a", int)], N)), "(int, {int a})");
}

#[test]
fn type_alias_is_written_only_with_prefer_type_alias() {
    let w = W::new();
    let c = core(&w);
    let ctx = w.ctx();
    // typedef A = int?;
    let int_q = w.iface(c.int, &[], Q);
    let alias_a: EId<TypeAliasElement> = w.store.add(TypeAliasElement {
        element: w.data(Some("A"), Tag::TypeAlias),
        type_params: vec![],
        aliased_type: VarSlot::with(int_q),
    });
    let with_alias = |n: Nullability| {
        let alias = ctx.intern_alias(AliasRef {
            element: alias_a,
            args: TypeList::EMPTY,
            nullability: n,
        });
        ctx.intern(TypeKind::Interface {
            element: c.int.upcast(),
            args: TypeList::EMPTY,
            nullability: Q,
            alias: Some(alias),
        })
    };
    let a = with_alias(N);
    let a_q = with_alias(Q);
    // `A x;` is an `int?` with the alias `A` without suffix; `A? x;` has `A?`.
    assert_eq!(type_display_string(&ctx, a, true), "A");
    assert_eq!(type_display_string(&ctx, a_q, true), "A?");
    assert_eq!(type_display_string(&ctx, a, false), "int?");
    assert_eq!(type_display_string(&ctx, a_q, false), "int?");

    // typedef L<T> = List<T>; L<String> (also for record and function types).
    let t = w.type_param(Some("T"));
    let alias_l: EId<TypeAliasElement> = w.store.add(TypeAliasElement {
        element: w.data(Some("L"), Tag::TypeAlias),
        type_params: vec![t],
        aliased_type: VarSlot::new(),
    });
    let string = w.iface(c.string, &[], N);
    let alias = ctx.intern_alias(AliasRef {
        element: alias_l,
        args: ctx.intern_list(&[string]),
        nullability: N,
    });
    let list_string = ctx.intern(TypeKind::Interface {
        element: c.list.upcast(),
        args: ctx.intern_list(&[string]),
        nullability: N,
        alias: Some(alias),
    });
    assert_eq!(type_display_string(&ctx, list_string, true), "L<String>");
    assert_eq!(
        type_display_string(&ctx, list_string, false),
        "List<String>"
    );
    let record = ctx.intern(TypeKind::Record {
        positional: ctx.intern_list(&[string]),
        named: NamedFields::EMPTY,
        nullability: N,
        alias: Some(alias),
    });
    assert_eq!(type_display_string(&ctx, record, true), "L<String>");
    assert_eq!(type_display_string(&ctx, record, false), "(String,)");
    let function = ctx.intern(TypeKind::Function(FunctionTypeData {
        type_params: TypeParamList::EMPTY,
        params: ParamList::EMPTY,
        required_positional: 0,
        ret: string,
        nullability: N,
        alias: Some(alias),
    }));
    assert_eq!(type_display_string(&ctx, function, true), "L<String>");
    assert_eq!(
        type_display_string(&ctx, function, false),
        "String Function()"
    );
    // The alias of a nested type is written too.
    let outer = w.iface(c.list, &[a_q], N);
    assert_eq!(type_display_string(&ctx, outer, true), "List<A?>");
}

#[test]
fn type_parameter_types_and_promoted_bounds() {
    let w = W::new();
    let c = core(&w);
    let t = w.type_param(Some("T"));
    let num = w.iface(c.num, &[], N);
    assert_eq!(w.ty(w.tpt(t, N)), "T");
    assert_eq!(w.ty(w.tpt(t, Q)), "T?");
    let promoted = |n| {
        w.ctx().intern(TypeKind::TypeParameter {
            param: t,
            nullability: n,
            promoted_bound: Some(num),
            alias: None,
        })
    };
    assert_eq!(w.ty(promoted(N)), "T & num");
    assert_eq!(w.ty(promoted(Q)), "(T & num)?");
    assert_eq!(w.ty(promoted(Star)), "(T & num)*");
}

#[test]
fn function_types() {
    let w = W::new();
    let c = core(&w);
    let int = w.iface(c.int, &[], N);
    let string = w.iface(c.string, &[], N);
    let boolean = w.iface(c.boolean, &[], N);
    assert_eq!(w.ty(w.func(&[], &[], TypeId::VOID, N)), "void Function()");
    assert_eq!(
        w.ty(w.func(
            &[],
            &[
                (Required, Some("s"), string),
                (Positional, Some("b"), boolean)
            ],
            int,
            N
        )),
        "int Function(String, [bool])"
    );
    assert_eq!(
        w.ty(w.func(
            &[],
            &[(NamedRequired, Some("a"), int), (Named, Some("b"), string)],
            TypeId::VOID,
            Q
        )),
        "void Function({required int a, String b})?"
    );
    assert_eq!(
        w.ty(w.func(
            &[],
            &[(Required, None, int), (Named, Some("b"), string)],
            TypeId::VOID,
            N
        )),
        "void Function(int, {String b})"
    );
    // Generic, with a bound; the variance of a formal is never written
    // (Dart writes synthetic type parameters).
    let num = w.iface(c.num, &[], N);
    let t = w.type_param_with("T", Some(Variance::Covariant), Some(num));
    let t_type = w.tpt(t, N);
    assert_eq!(
        w.ty(w.func(&[t], &[(Required, None, t_type)], t_type, N)),
        "T Function<T extends num>(T)"
    );
}

#[test]
fn function_type_formals_shadow_without_renaming() {
    let w = W::new();
    let c = core(&w);
    // List<T> Function<T>(T): only the own T is referenced.
    let t = w.type_param(Some("T"));
    let t_type = w.tpt(t, N);
    let list_t = w.iface(c.list, &[t_type], N);
    assert_eq!(
        w.ty(w.func(&[t], &[(Required, None, t_type)], list_t, N)),
        "List<T> Function<T>(T)"
    );
}

#[test]
fn function_type_formals_are_renamed_when_names_clash() {
    let w = W::new();
    let c = core(&w);
    // U is another element that is also named `T` (for example a type
    // parameter of the enclosing class).
    let outer_t = w.type_param(Some("T"));
    let outer_t_type = w.tpt(outer_t, N);
    let t = w.type_param(Some("T"));
    let t_type = w.tpt(t, N);
    let f = w.func(
        &[t],
        &[(Required, None, t_type), (Required, None, outer_t_type)],
        TypeId::VOID,
        N,
    );
    assert_eq!(w.ty(f), "void Function<T₀>(T₀, T)");

    // The rename also applies inside nested types and the return type.
    let list_t = w.iface(c.list, &[t_type], Q);
    let g = w.func(&[t], &[(Named, Some("x"), outer_t_type)], list_t, N);
    assert_eq!(w.ty(g), "List<T₀>? Function<T₀>({T x})");

    // Duplicate names among the formals are renamed too.
    let t2 = w.type_param(Some("T"));
    let t2_type = w.tpt(t2, N);
    let h = w.func(
        &[t, t2],
        &[(Required, None, t_type), (Required, None, t2_type)],
        TypeId::VOID,
        N,
    );
    assert_eq!(w.ty(h), "void Function<T, T₀>(T, T₀)");

    // The counter goes on while the name is taken: T and T₀ are referenced.
    let outer_t0 = w.type_param(Some("T₀"));
    let outer_t0_type = w.tpt(outer_t0, N);
    let k = w.func(
        &[t],
        &[
            (Required, None, t_type),
            (Required, None, outer_t_type),
            (Required, None, outer_t0_type),
        ],
        TypeId::VOID,
        N,
    );
    assert_eq!(w.ty(k), "void Function<T₁>(T₁, T, T₀)");
}

#[test]
fn function_type_formal_bounds_keep_the_outer_names() {
    let w = W::new();
    let c = core(&w);
    let outer_t = w.type_param(Some("T"));
    let outer_t_type = w.tpt(outer_t, N);

    // void Function<T extends U>(T) with U named `T`: the bound is copied
    // without substitution, so it prints the outer name.
    let t = w.type_param_with("T", None, Some(outer_t_type));
    let t_type = w.tpt(t, N);
    let f = w.func(&[t], &[(Required, None, t_type)], TypeId::VOID, N);
    assert_eq!(w.ty(f), "void Function<T₀ extends T>(T₀)");

    // An F-bound on a renamed formal keeps the old name (Dart output).
    let t_f = w.type_param(Some("T"));
    let t_f_type = w.tpt(t_f, N);
    let comparable = w.class_in(w.core, "Comparable", vec![w.type_param(Some("T"))]);
    let bound = w.iface(comparable, &[t_f_type], N);
    w.ctx().get(t_f).bound.set(Some(bound));
    let g = w.func(
        &[t_f],
        &[(Required, None, t_f_type), (Required, None, outer_t_type)],
        TypeId::VOID,
        N,
    );
    assert_eq!(w.ty(g), "void Function<T₀ extends Comparable<T>>(T₀, T)");

    // A bound of a nested formal that refers to a renamed outer formal uses
    // the new name: the outer substitution reaches nested function types.
    let s = w.type_param_with("S", None, Some(t_f_type));
    let s_type = w.tpt(s, N);
    let inner = w.func(&[s], &[(Required, None, s_type)], TypeId::VOID, N);
    let outer = w.func(
        &[t_f],
        &[
            (Required, None, t_f_type),
            (Required, None, outer_t_type),
            (Required, None, inner),
        ],
        TypeId::VOID,
        N,
    );
    assert_eq!(
        w.ty(outer),
        "void Function<T₀ extends Comparable<T>>(T₀, T, void Function<S extends T₀>(S))"
    );
    let _ = c;
}

#[test]
fn nested_function_type_renames_against_outer_new_names() {
    let w = W::new();
    // void Function<T>(T, void Function<T>(T, T)) where the nested function
    // type references the outer formal.
    let a = w.type_param(Some("T"));
    let a_type = w.tpt(a, N);
    let b = w.type_param(Some("T"));
    let b_type = w.tpt(b, N);
    let inner = w.func(
        &[b],
        &[(Required, None, b_type), (Required, None, a_type)],
        TypeId::VOID,
        N,
    );
    let outer = w.func(
        &[a],
        &[(Required, None, a_type), (Required, None, inner)],
        TypeId::VOID,
        N,
    );
    // The outer formal avoids the name of the nested formal (collected
    // through the nested function type), the nested one avoids `T₀`.
    assert_eq!(
        w.ty(outer),
        "void Function<T₀>(T₀, void Function<T>(T, T₀))"
    );
}

// ---------------------------------------------------------------- elements

#[test]
fn class_elements_with_modifiers_and_supertypes() {
    let w = W::new();
    let c = core(&w);
    let ctx = w.ctx();
    let object = w.iface(c.object, &[], N);

    let plain = w.class("C", vec![]);
    ctx.get(plain).supertype.set(Some(object));
    assert_eq!(w.el(plain.raw()), "class C");

    let b = w.class("B", vec![]);
    let i = w.class("I", vec![]);
    let m = w.class("M", vec![]);
    let t = w.type_param(Some("T"));
    let a = w.class("A", vec![t]);
    let ad = ctx.get(a);
    ad.flags.set(ElementFlags::CLASS_ELEMENT_IS_ABSTRACT, true);
    ad.supertype.set(Some(w.iface(b, &[], N)));
    ad.mixins.set(Some(ctx.intern_list(&[w.iface(m, &[], N)])));
    let t_type = w.tpt(t, N);
    ad.interfaces.set(Some(
        ctx.intern_list(&[w.iface(i, &[], N), w.iface(c.list, &[t_type], N)]),
    ));
    assert_eq!(
        w.el(a.raw()),
        "abstract class A<T> extends B with M implements I, List<T>"
    );

    // sealed wins over abstract; base, interface, final; mixin class.
    let s = w.class("S", vec![]);
    let sd = ctx.get(s);
    sd.flags.set(ElementFlags::CLASS_ELEMENT_IS_ABSTRACT, true);
    ctx.fragment(sd.first_fragment())
        .flags
        .set(FragmentFlags::CLASS_FRAGMENT_IS_SEALED, true);
    assert_eq!(w.el(s.raw()), "sealed class S");

    let f = w.class("F", vec![]);
    ctx.get(f)
        .flags
        .set(ElementFlags::CLASS_ELEMENT_IS_FINAL, true);
    assert_eq!(w.el(f.raw()), "final class F");

    let n = w.class("N", vec![]);
    ctx.get(n)
        .flags
        .set(ElementFlags::CLASS_ELEMENT_IS_INTERFACE, true);
    assert_eq!(w.el(n.raw()), "interface class N");

    let bm = w.class("BM", vec![]);
    let bmd = ctx.get(bm);
    bmd.flags.set(ElementFlags::CLASS_ELEMENT_IS_BASE, true);
    bmd.flags.set(ElementFlags::CLASS_ELEMENT_IS_ABSTRACT, true);
    ctx.fragment(bmd.first_fragment())
        .flags
        .set(FragmentFlags::CLASS_FRAGMENT_IS_MIXIN_CLASS, true);
    assert_eq!(w.el(bm.raw()), "abstract base mixin class BM");

    // `Object` of another library is written.
    let my_object = w.class("Object", vec![]);
    let d = w.class("D", vec![]);
    ctx.get(d).supertype.set(Some(w.iface(my_object, &[], N)));
    assert_eq!(w.el(d.raw()), "class D extends Object");
}

#[test]
fn enum_mixin_extension_and_extension_type_elements() {
    let w = W::new();
    let c = core(&w);
    let i = w.class("I", vec![]);
    let m = w.class("M", vec![]);
    let a = w.class("A", vec![]);

    // enum E with M implements I
    let e: EId<EnumElement> = {
        let iface = InterfaceElementData::new(w.data(Some("E"), Tag::Enum));
        iface
            .mixins
            .set(Some(w.ctx().intern_list(&[w.iface(m, &[], N)])));
        iface
            .interfaces
            .set(Some(w.ctx().intern_list(&[w.iface(i, &[], N)])));
        w.store.add(EnumElement { interface: iface })
    };
    assert_eq!(w.el(e.raw()), "enum E with M implements I");

    // base mixin X<T> on A implements I
    let t = w.type_param(Some("T"));
    let mixin_fragment = w.store.add_fragment::<MixinFragment>(MixinFragment {
        interface: InterfaceFragmentData::new(FragmentData::new(Some(w.name("X")), None)),
        super_invoked_names: OnceSlot::new(),
    });
    w.store
        .fragment(mixin_fragment)
        .flags
        .set(FragmentFlags::MIXIN_FRAGMENT_IS_BASE, true);
    let mut iface =
        InterfaceElementData::new(ElementData::new(Some(w.name("X")), mixin_fragment.raw()));
    iface.instance.type_params = vec![t];
    iface
        .interfaces
        .set(Some(w.ctx().intern_list(&[w.iface(i, &[], N)])));
    let x: EId<MixinElement> = w.store.add(MixinElement {
        interface: iface,
        superclass_constraints: VarSlot::with(w.ctx().intern_list(&[w.iface(a, &[], N)])),
    });
    assert_eq!(w.el(x.raw()), "base mixin X<T> on A implements I");

    // extension Ext<T> on List<T>; extension on int
    let t = w.type_param(Some("T"));
    let list_t = w.iface(c.list, &[w.tpt(t, N)], N);
    let mut inst = InstanceElementData::new(w.data(Some("Ext"), Tag::Extension));
    inst.type_params = vec![t];
    let ext: EId<ExtensionElement> = w.store.add(ExtensionElement {
        instance: inst,
        extended_type: VarSlot::with(list_t),
    });
    assert_eq!(w.el(ext.raw()), "extension Ext<T> on List<T>");
    let unnamed: EId<ExtensionElement> = w.store.add(ExtensionElement {
        instance: InstanceElementData::new(w.data(None, Tag::Extension)),
        extended_type: VarSlot::with(w.iface(c.int, &[], N)),
    });
    assert_eq!(w.el(unnamed.raw()), "extension on int");
    let no_type: EId<ExtensionElement> = w.store.add(ExtensionElement {
        instance: InstanceElementData::new(w.data(Some("Z"), Tag::Extension)),
        extended_type: VarSlot::new(),
    });
    assert_eq!(w.el(no_type.raw()), "extension Z on InvalidType");

    // extension type ET<T>(List<T> it) implements Iterable<T>
    let t = w.type_param(Some("T"));
    let t_type = w.tpt(t, N);
    let iterable = w.class_in(w.core, "Iterable", vec![w.type_param(Some("E"))]);
    let field: EId<FieldElement> = {
        let p = PropertyInducingElementData::new(w.data(Some("it"), Tag::Field));
        p.type_.set(Some(w.iface(c.list, &[t_type], N)));
        w.store.add(FieldElement { property: p })
    };
    let mut iface = InterfaceElementData::new(w.data(Some("ET"), Tag::ExtensionType));
    iface.instance.type_params = vec![t];
    iface.instance.fields = vec![field];
    iface.interfaces.set(Some(w.ctx().intern_list(&[w.iface(
        iterable,
        &[t_type],
        N,
    )])));
    let et: EId<ExtensionTypeElement> = w.store.add(ExtensionTypeElement {
        interface: iface,
        has_representation_self_reference: BoolSlot::new(false),
        has_implements_self_reference: BoolSlot::new(false),
        type_erasure: OnceSlot::new(),
    });
    assert_eq!(
        w.el(et.raw()),
        "extension type ET<T>(List<T> it) implements Iterable<T>"
    );
    // The field itself is a variable element.
    assert_eq!(w.el(field.raw()), "List<T> it");
}

#[test]
fn executable_elements() {
    let w = W::new();
    let c = core(&w);
    let ctx = w.ctx();
    let int = w.iface(c.int, &[], N);
    let int_q = w.iface(c.int, &[], Q);
    let string_q = w.iface(c.string, &[], Q);

    // int foo<T extends num>(T a, [String? b])
    let num = w.iface(c.num, &[], N);
    let t = w.type_param_with("T", None, Some(num));
    let a = w.param(Some("a"), Required, Some(w.tpt(t, N)));
    let b = w.param(Some("b"), Positional, Some(string_q));
    let foo = w.method("foo", vec![t], vec![a, b], int);
    assert_eq!(w.el(foo.raw()), "int foo<T extends num>(T a, [String? b])");

    // void m({required int a, int? b}) (the default value is not written).
    let pa = w.param(Some("a"), NamedRequired, Some(int));
    let pb = w.param(Some("b"), Named, Some(int_q));
    let m = w.method("m", vec![], vec![pa, pb], TypeId::VOID);
    assert_eq!(w.el(m.raw()), "void m({required int a, int? b})");

    // Getter and setter.
    let getter: EId<GetterElement> = w.store.add(GetterElement {
        accessor: PropertyAccessorElementData {
            executable: w.executable(Some("x"), Tag::Getter, vec![], vec![], Some(int)),
            variable: VarSlot::new(),
        },
    });
    assert_eq!(w.el(getter.raw()), "int get x");
    let value = w.param(Some("value"), Required, Some(int));
    let setter: EId<SetterElement> = w.store.add(SetterElement {
        accessor: PropertyAccessorElementData {
            executable: w.executable(
                Some("x"),
                Tag::Setter,
                vec![],
                vec![value],
                Some(TypeId::VOID),
            ),
            variable: VarSlot::new(),
        },
    });
    assert_eq!(w.el(setter.raw()), "set x(int value)");

    // Constructors: the return type is the `thisType` of the class.
    let k = w.type_param(Some("K"));
    let class = w.class("C", vec![k]);
    let p = w.param(Some("p"), Required, Some(w.tpt(k, N)));
    let constructor = |name: &str, params: Vec<EId<FormalParameterElement>>| {
        let mut e = w.executable(Some(name), Tag::Constructor, vec![], params, None);
        e.element.enclosing = Some(class.raw());
        let id: EId<ConstructorElement> = w.store.add(ConstructorElement {
            executable: e,
            redirected_constructor: VarSlot::new(),
            super_constructor: VarSlot::new(),
        });
        id
    };
    let named = constructor("named", vec![p]);
    assert_eq!(w.el(named.raw()), "C<K>.named(K p)");
    let unnamed = constructor("new", vec![]);
    assert_eq!(w.el(unnamed.raw()), "C<K>()");
    // With a set return type.
    ctx.get(unnamed)
        .return_type
        .set(Some(w.iface(class, &[int], N)));
    assert_eq!(w.el(unnamed.raw()), "C<int>()");

    // Top-level function and local function.
    let tf: EId<TopLevelFunctionElement> = w.store.add(TopLevelFunctionElement {
        executable: w.executable(
            Some("main"),
            Tag::TopLevelFunction,
            vec![],
            vec![],
            Some(TypeId::VOID),
        ),
    });
    assert_eq!(w.el(tf.raw()), "void main()");
    let lf: EId<LocalFunctionElement> = w.store.add(LocalFunctionElement {
        executable: w.executable(Some("local"), Tag::LocalFunction, vec![], vec![], None),
    });
    assert_eq!(w.el(lf.raw()), "InvalidType local()");

    // Generic function type element: names of positional parameters too.
    let s = w.type_param(Some("S"));
    let gp = w.param(Some("s"), Required, Some(w.tpt(s, N)));
    let gft: EId<GenericFunctionTypeElement> = w.store.add(GenericFunctionTypeElement {
        element: w.data(None, Tag::GenericFunctionType),
        type_params: vec![s],
        formal_params: vec![gp],
        return_type: VarSlot::with(int),
        type_: VarSlot::new(),
    });
    assert_eq!(w.el(gft.raw()), "int Function<S>(S s)");
}

#[test]
fn multiline_formal_parameters() {
    let w = W::new();
    let c = core(&w);
    let string_q = w.iface(c.string, &[], Q);
    let int = w.iface(c.int, &[], N);
    let ml = DisplayOptions {
        multiline: true,
        prefer_type_alias: false,
    };

    let aaa = w.param(Some("aaa"), Required, Some(string_q));
    let bbb = w.param(Some("bbb"), Positional, Some(string_q));
    let ccc = w.param(Some("ccc"), Positional, Some(string_q));
    let m = w.method("longMethodName", vec![], vec![aaa, bbb, ccc], string_q);
    assert_eq!(
        element_display_string_with(&w.ctx(), m.raw(), ml),
        "String? longMethodName(\n  String? aaa, [\n  String? bbb,\n  String? ccc,\n])"
    );
    // Without `multiline` the same method is on one line.
    assert_eq!(
        w.el(m.raw()),
        "String? longMethodName(String? aaa, [String? bbb, String? ccc])"
    );

    // Named group.
    let a = w.param(Some("a"), Required, Some(int));
    let b = w.param(Some("b"), NamedRequired, Some(int));
    let cc = w.param(Some("c"), Named, Some(int));
    let f: EId<TopLevelFunctionElement> = w.store.add(TopLevelFunctionElement {
        executable: w.executable(
            Some("f"),
            Tag::TopLevelFunction,
            vec![],
            vec![a, b, cc],
            Some(TypeId::VOID),
        ),
    });
    assert_eq!(
        element_display_string_with(&w.ctx(), f.raw(), ml),
        "void f(\n  int a, {\n  required int b,\n  int c,\n})"
    );

    // Only named parameters: the open group follows `(` directly.
    let n1 = w.param(Some("a"), Named, Some(int));
    let n2 = w.param(Some("b"), Named, Some(int));
    let n3 = w.param(Some("c"), Named, Some(int));
    let g = w.method("g", vec![], vec![n1, n2, n3], TypeId::VOID);
    assert_eq!(
        element_display_string_with(&w.ctx(), g.raw(), ml),
        "void g({\n  int a,\n  int b,\n  int c,\n})"
    );

    // Two parameters stay on one line.
    let two = w.method("two", vec![], vec![aaa, bbb], TypeId::VOID);
    assert_eq!(
        element_display_string_with(&w.ctx(), two.raw(), ml),
        "void two(String? aaa, [String? bbb])"
    );

    // Local functions never wrap.
    let lf: EId<LocalFunctionElement> = w.store.add(LocalFunctionElement {
        executable: w.executable(
            Some("lf"),
            Tag::LocalFunction,
            vec![],
            vec![aaa, bbb, ccc],
            Some(int),
        ),
    });
    assert_eq!(
        element_display_string_with(&w.ctx(), lf.raw(), ml),
        "int lf(String? aaa, [String? bbb, String? ccc])"
    );
}

#[test]
fn type_alias_type_parameter_and_parameter_elements() {
    let w = W::new();
    let c = core(&w);
    let ctx = w.ctx();
    let num = w.iface(c.num, &[], N);
    let int = w.iface(c.int, &[], N);

    // typedef A<T> = List<T>
    let t = w.type_param(Some("T"));
    let list_t = w.iface(c.list, &[w.tpt(t, N)], N);
    let a: EId<TypeAliasElement> = w.store.add(TypeAliasElement {
        element: w.data(Some("A"), Tag::TypeAlias),
        type_params: vec![t],
        aliased_type: VarSlot::with(list_t),
    });
    assert_eq!(w.el(a.raw()), "typedef A<T> = List<T>");
    // typedef F = void Function(int)
    let f: EId<TypeAliasElement> = w.store.add(TypeAliasElement {
        element: w.data(Some("F"), Tag::TypeAlias),
        type_params: vec![],
        aliased_type: VarSlot::with(w.func(&[], &[(Required, Some("x"), int)], TypeId::VOID, N)),
    });
    assert_eq!(w.el(f.raw()), "typedef F = void Function(int)");

    // Type parameters: variance keyword only when declared and not unrelated.
    let out_t = w.type_param_with("T", Some(Variance::Covariant), Some(num));
    assert_eq!(w.el(out_t.raw()), "out T extends num");
    assert_eq!(
        type_parameter_display_string(&ctx, out_t),
        "out T extends num"
    );
    let in_t = w.type_param_with("T", Some(Variance::Contravariant), None);
    assert_eq!(w.el(in_t.raw()), "in T");
    let inout_t = w.type_param_with("T", Some(Variance::Invariant), None);
    assert_eq!(w.el(inout_t.raw()), "inout T");
    let unrelated = w.type_param_with("T", Some(Variance::Unrelated), Some(num));
    assert_eq!(w.el(unrelated.raw()), "T extends num");
    let legacy = w.type_param_with("T", None, None);
    assert_eq!(w.el(legacy.raw()), "T");
    // The class lists its type parameters with variance.
    let class = w.class("V", vec![out_t, legacy]);
    assert_eq!(w.el(class.raw()), "class V<out T extends num, T>");

    // Formal parameter elements with delimiters.
    assert_eq!(w.el(w.param(Some("a"), Required, Some(int)).raw()), "int a");
    assert_eq!(
        w.el(w.param(Some("b"), Positional, Some(int)).raw()),
        "[int b]"
    );
    assert_eq!(w.el(w.param(Some("c"), Named, Some(int)).raw()), "{int c}");
    assert_eq!(
        w.el(w.param(Some("d"), NamedRequired, Some(int)).raw()),
        "{required int d}"
    );
    assert_eq!(
        w.el(w.param(Some("e"), Required, None).raw()),
        "InvalidType e"
    );
}

#[test]
fn variable_library_label_prefix_and_special_elements() {
    let mut w = W::new();
    let c = core(&w);
    let int = w.iface(c.int, &[], N);

    let v: EId<TopLevelVariableElement> = {
        let p = PropertyInducingElementData::new(w.data(Some("x"), Tag::TopLevelVariable));
        p.type_.set(Some(int));
        w.store.add(TopLevelVariableElement { property: p })
    };
    assert_eq!(w.el(v.raw()), "int x");
    let local: EId<LocalVariableElement> = w.store.add(LocalVariableElement {
        variable: VariableElementData::new(w.data(Some("y"), Tag::LocalVariable)),
        type_: VarSlot::new(),
    });
    assert_eq!(w.el(local.raw()), "InvalidType y");

    assert_eq!(w.el(w.lib.raw()), "library package:p/a.dart");
    assert_eq!(w.el(w.core.raw()), "library dart:core");

    let label: EId<LabelElement> = w.store.add(LabelElement {
        element: w.data(Some("outer"), Tag::Label),
    });
    assert_eq!(w.el(label.raw()), "outer");

    assert_eq!(w.el(ElementId::DYNAMIC), "dynamic");
    assert_eq!(w.el(ElementId::NEVER), "Never");

    // import 'a.dart' as p; import 'b.dart' as p; (not the import as q).
    let prefix_fragment = |name: &str| {
        let mut f = FragmentData::new(Some(w.name(name)), None);
        f.enclosing_fragment = Some(w.unit.raw());
        w.store.add_fragment::<PrefixFragment>(PrefixFragment {
            fragment: f,
            offset: 0,
            is_deferred: false,
        })
    };
    let pf = prefix_fragment("p");
    let qf = prefix_fragment("q");
    let prefix_element = |f: FId<PrefixFragment>, name: &str| {
        let p: EId<PrefixElement> = w.store.add(PrefixElement {
            element: ElementData::new(Some(w.name(name)), f.raw()),
            local_id: w.name(name),
            last_fragment: f,
        });
        w.store.fragment(f).element.set_once(p.raw());
        p
    };
    let p = prefix_element(pf, "p");
    let q = prefix_element(qf, "q");
    let unit = w.unit;
    let import = |uri: &str, prefix: FId<PrefixFragment>| LibraryImport {
        directive: ElementDirective {
            library_fragment: unit,
            uri: DirectiveUri::RelativeUriString {
                relative_uri_string: Arc::from(uri),
            },
            metadata: Metadata::default(),
        },
        is_synthetic: false,
        combinators: vec![],
        import_keyword_offset: 0,
        prefix: Some(prefix),
        namespace: OnceSlot::new(),
    };
    let imports = vec![
        import("a.dart", pf),
        import("c.dart", qf),
        import("b.dart", pf),
    ];
    w.store.fragment_mut(unit).library_imports = imports;
    assert_eq!(
        w.el(p.raw()),
        "import 'a.dart' as p;\nimport 'b.dart' as p;"
    );
    assert_eq!(w.el(q.raw()), "import 'c.dart' as q;");
}

#[test]
fn diagnostic_type_argument_uses_prefer_type_alias() {
    let w = W::new();
    let c = core(&w);
    let ctx = w.ctx();
    let int = w.iface(c.int, &[], N);
    let alias_a: EId<TypeAliasElement> = w.store.add(TypeAliasElement {
        element: w.data(Some("A"), Tag::TypeAlias),
        type_params: vec![],
        aliased_type: VarSlot::with(int),
    });
    let alias = ctx.intern_alias(AliasRef {
        element: alias_a,
        args: TypeList::EMPTY,
        nullability: N,
    });
    let a = ctx.intern(TypeKind::Interface {
        element: c.int.upcast(),
        args: TypeList::EMPTY,
        nullability: N,
        alias: Some(alias),
    });
    // `convertTypeNames` calls `getDisplayString(preferTypeAlias: true)`.
    assert_eq!(diagnostics::type_arg(&ctx, a).display, "A");
    assert_eq!(
        diagnostics::element_arg(&ctx, c.int.raw()).display,
        "class int"
    );
}

//! Test support: a small hand-built `dart:core` / `dart:async` world with a
//! filled [`TypeProvider`], and [`TestTypeSystem`], a stand-in for the
//! `dartr_typesystem` implementation of [`ConstTypeSystem`] that is exact
//! for the types these tests use (interface types of the core classes,
//! `FutureOr`, nullable types, simple function types).

#![allow(dead_code)]

use std::sync::Arc;

use dartr_constant::{
    BoolState, ConstTypeSystem, DartObjectImpl, DartObjectMap, DartObjectSet, DoubleState,
    EvalResult, FieldMap, InstanceState, IntState, ListState, MapState, NullState, RecordState,
    SetState, StringState, SymbolState, TypeState,
};
use dartr_element::*;

pub struct World {
    pub world: WorldSnapshot,
    pub tp: TypeProvider,
    pub features: FeatureSet,
    /// The type parameters `T` and `S extends T` of a class `_Scope<T, S>`
    /// (Dart `withTypeParameterScope`).
    pub scope_t: EId<TypeParameterElement>,
    pub scope_s: EId<TypeParameterElement>,
}

fn add_library(
    store: &ElementStore,
    names: &NamePool,
    uri: &str,
    index: u32,
) -> (EId<LibraryElement>, FId<LibraryFragment>) {
    let unit = FId::<LibraryFragment>::from_raw(FragmentId::new(store.id, Tag::Library, index));
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
        path: Arc::from(format!("/sdk/{index}.dart")),
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

fn add_class(
    store: &mut ElementStore,
    names: &NamePool,
    library: EId<LibraryElement>,
    unit: FId<LibraryFragment>,
    name: &str,
    type_params: &[&str],
) -> EId<ClassElement> {
    let mut fragment = FragmentData::new(Some(names.intern(name)), Some(0));
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
    for &tp_name in type_params {
        let mut f = FragmentData::new(Some(names.intern(tp_name)), Some(0));
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
    store.get_mut(class).type_params = params;
    store.get_mut(library).classes.push(class);
    store.fragment_mut(unit).classes.push(class_fragment);
    class
}

/// Builds `dart:core` (Object, bool, num, int, double, String, Null, List,
/// Map, Set, Symbol, Type, Record, Function) and `dart:async` (FutureOr)
/// in one frozen cycle store, and fills the type provider.
pub fn build_world() -> World {
    let generation = Arc::new(Generation::new(0));
    let names = &generation.names;
    let tp = TypeProvider::default();
    let features = FeatureSet::default();

    let mut store = generation.new_cycle_store();
    let (core, core_unit) = add_library(&store, names, "dart:core", 0);
    let (async_, async_unit) = add_library(&store, names, "dart:async", 1);
    let mut class =
        |name: &str, params: &[&str]| add_class(&mut store, names, core, core_unit, name, params);
    let object = class("Object", &[]);
    let bool_ = class("bool", &[]);
    let num = class("num", &[]);
    let int = class("int", &[]);
    let double = class("double", &[]);
    let string = class("String", &[]);
    let null = class("Null", &[]);
    let list = class("List", &["E"]);
    let map = class("Map", &["K", "V"]);
    let set = class("Set", &["E"]);
    let symbol = class("Symbol", &[]);
    let type_ = class("Type", &[]);
    let record = class("Record", &[]);
    let function = class("Function", &[]);
    let scope = class("_Scope", &["T", "S"]);
    let future_or = add_class(&mut store, names, async_, async_unit, "FutureOr", &["T"]);
    let (scope_t, scope_s) = {
        let params = &store.get(scope).type_params;
        (params[0], params[1])
    };

    let store = Arc::new(store);
    let world = WorldSnapshot::new(generation.clone()).with_store(
        store,
        [
            (Arc::from("dart:core"), core),
            (Arc::from("dart:async"), async_),
        ],
    );
    {
        let ctx = Ctx {
            world: &world,
            current: None,
            local: None,
            tp: &tp,
            features: &features,
            req: &NoopSink,
        };
        let interface = |e: EId<ClassElement>, nullability: Nullability| {
            let args = ctx.intern_list::<TypeId>(&[]);
            ctx.intern(TypeKind::Interface {
                element: e.upcast(),
                args,
                nullability,
                alias: None,
            })
        };
        tp.core_library.set_once(core);
        tp.async_library.set_once(async_);
        tp.object_element.set_once(object);
        tp.bool_element.set_once(bool_);
        tp.num_element.set_once(num);
        tp.int_element.set_once(int);
        tp.double_element.set_once(double);
        tp.string_element.set_once(string);
        tp.null_element.set_once(null);
        tp.list_element.set_once(list);
        tp.map_element.set_once(map);
        tp.set_element.set_once(set);
        tp.symbol_element.set_once(symbol);
        tp.type_element.set_once(type_);
        tp.record_element.set_once(record);
        tp.function_element.set_once(function);
        tp.future_or_element.set_once(future_or);
        tp.object_type
            .set_once(interface(object, Nullability::None));
        tp.object_question_type
            .set_once(interface(object, Nullability::Question));
        tp.bool_type.set_once(interface(bool_, Nullability::None));
        tp.num_type.set_once(interface(num, Nullability::None));
        tp.int_type.set_once(interface(int, Nullability::None));
        tp.double_type
            .set_once(interface(double, Nullability::None));
        tp.string_type
            .set_once(interface(string, Nullability::None));
        tp.null_type.set_once(interface(null, Nullability::None));
        tp.symbol_type
            .set_once(interface(symbol, Nullability::None));
        tp.type_type.set_once(interface(type_, Nullability::None));
        tp.record_type
            .set_once(interface(record, Nullability::None));
        tp.function_type
            .set_once(interface(function, Nullability::None));
        let t_type = ctx.intern(TypeKind::TypeParameter {
            param: scope_t,
            nullability: Nullability::None,
            promoted_bound: None,
            alias: None,
        });
        ctx.get(scope_s).bound.set(Some(t_type));
    }
    World {
        world,
        tp,
        features,
        scope_t,
        scope_s,
    }
}

impl World {
    pub fn ctx(&self) -> Ctx<'_> {
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

/// A stand-in for the `dartr_typesystem` implementation of the hook.
///
/// - `normalize`: `FutureOr<T>` is `T` when `T` is `Object` (the only
///   `FutureOr` rule these tests need), applied to type arguments.
/// - `types_equal` (Dart `TypeImpl.==`): structural equality without the
///   alias.
/// - `runtime_types_equal`: equality of the normalized types.
/// - `is_subtype_of`: reflexive, top types and `Null` to nullable types.
/// - `display_string`: the default display string of these types.
pub struct TestTypeSystem<'w> {
    pub world: &'w World,
}

impl TestTypeSystem<'_> {
    fn interface_name(&self, element: EId<InterfaceElement>) -> String {
        let ctx = self.ctx();
        let name = ctx.element_data(element.raw()).unwrap().name.unwrap();
        ctx.name_str(name).to_string()
    }

    fn strip_alias(&self, t: TypeId) -> TypeKind {
        let ctx = self.ctx();
        match *ctx.ty(t) {
            TypeKind::Interface {
                element,
                args,
                nullability,
                ..
            } => TypeKind::Interface {
                element,
                args,
                nullability,
                alias: None,
            },
            TypeKind::Function(f) => TypeKind::Function(FunctionTypeData { alias: None, ..f }),
            TypeKind::Record {
                positional,
                named,
                nullability,
                ..
            } => TypeKind::Record {
                positional,
                named,
                nullability,
                alias: None,
            },
            other => other,
        }
    }
}

impl ConstTypeSystem for TestTypeSystem<'_> {
    fn ctx(&self) -> Ctx<'_> {
        self.world.ctx()
    }

    fn is_subtype_of(&self, left: TypeId, right: TypeId) -> bool {
        let ctx = self.ctx();
        if self.runtime_types_equal(left, right) {
            return true;
        }
        if right == TypeId::DYNAMIC || right == ctx.tp.object_question_type() {
            return true;
        }
        if right == ctx.tp.object_type() {
            return left != ctx.tp.null_type();
        }
        if left == ctx.tp.null_type() {
            return matches!(
                ctx.ty(right),
                TypeKind::Interface {
                    nullability: Nullability::Question,
                    ..
                }
            );
        }
        if (left == ctx.tp.int_type() || left == ctx.tp.double_type()) && right == ctx.tp.num_type()
        {
            return true;
        }
        false
    }

    fn runtime_types_equal(&self, t1: TypeId, t2: TypeId) -> bool {
        self.types_equal(self.normalize(t1), self.normalize(t2))
    }

    fn normalize(&self, t: TypeId) -> TypeId {
        let ctx = self.ctx();
        match *ctx.ty(t) {
            TypeKind::Interface {
                element,
                args,
                nullability,
                alias,
            } => {
                let new_args: Vec<TypeId> =
                    ctx.list(args).iter().map(|&a| self.normalize(a)).collect();
                if element.raw() == ctx.tp.future_or_element().raw()
                    && nullability == Nullability::None
                    && new_args[0] == ctx.tp.object_type()
                {
                    return new_args[0];
                }
                let args = ctx.intern_list(&new_args);
                ctx.intern(TypeKind::Interface {
                    element,
                    args,
                    nullability,
                    alias,
                })
            }
            _ => t,
        }
    }

    fn types_equal(&self, t1: TypeId, t2: TypeId) -> bool {
        // Dart: ==
        t1 == t2 || self.strip_alias(t1) == self.strip_alias(t2)
    }

    fn extension_type_erasure(&self, t: TypeId) -> TypeId {
        t
    }

    fn display_string(&self, t: TypeId) -> String {
        let ctx = self.ctx();
        let suffix = |n: Nullability| if n == Nullability::Question { "?" } else { "" };
        match *ctx.ty(t) {
            TypeKind::Dynamic => "dynamic".to_string(),
            TypeKind::Void => "void".to_string(),
            TypeKind::Invalid => "InvalidType".to_string(),
            TypeKind::Unknown => "_".to_string(),
            TypeKind::Never(n) => format!("Never{}", suffix(n)),
            TypeKind::Interface {
                element,
                args,
                nullability,
                ..
            } => {
                let mut s = self.interface_name(element);
                let args = ctx.list(args);
                if !args.is_empty() {
                    let parts: Vec<String> = args.iter().map(|&a| self.display_string(a)).collect();
                    s = format!("{s}<{}>", parts.join(", "));
                }
                format!("{s}{}", suffix(nullability))
            }
            TypeKind::Function(f) => {
                let params: Vec<String> = ctx
                    .list(f.params)
                    .iter()
                    .map(|p| self.display_string(p.ty))
                    .collect();
                let s = format!(
                    "{} Function({})",
                    self.display_string(f.ret),
                    params.join(", ")
                );
                if f.nullability == Nullability::Question {
                    format!("{s}?")
                } else {
                    s
                }
            }
            TypeKind::Record {
                positional,
                named,
                nullability,
                ..
            } => {
                let mut parts: Vec<String> = ctx
                    .list(positional)
                    .iter()
                    .map(|&p| self.display_string(p))
                    .collect();
                let named: Vec<String> = ctx
                    .list(named)
                    .iter()
                    .map(|n| format!("{} {}", self.display_string(n.ty), ctx.name_str(n.name)))
                    .collect();
                if !named.is_empty() {
                    parts.push(format!("{{{}}}", named.join(", ")));
                }
                format!("({}){}", parts.join(", "), suffix(nullability))
            }
            TypeKind::TypeParameter { param, .. } => {
                let name = ctx.get(param).name.unwrap();
                ctx.name_str(name).to_string()
            }
        }
    }

    fn look_up_concrete_method(
        &self,
        _ty: TypeId,
        _name: &str,
        _library: EId<LibraryElement>,
    ) -> Option<ElemRef> {
        None
    }

    fn look_up_concrete_getter(
        &self,
        _ty: TypeId,
        _name: &str,
        _library: EId<LibraryElement>,
    ) -> Option<ElemRef> {
        None
    }
}

/// The fixture of `DartObjectImplTest` (value_test.dart): the value
/// builders and the `_assert*` helpers.
pub struct T {
    pub world: Box<World>,
    pub feature_set: FeatureSet,
}

pub const LONG_MAX_VALUE: i64 = 0x7fffffffffffffff;

macro_rules! binary_asserts {
    ($($name:ident => $op:ident,)*) => {
        $(
            /// Dart `_assert*`: the operation gives [expected], or throws an
            /// `EvaluationException` when [expected] is `None`. An
            /// unexpected exception is returned (Dart rethrows it).
            pub fn $name(
                &self,
                expected: Option<DartObjectImpl>,
                left: DartObjectImpl,
                right: DartObjectImpl,
            ) -> EvalResult<()> {
                let ts = self.ts();
                self.check(expected, || left.$op(&ts, &right))
            }
        )*
    };
}

macro_rules! unary_asserts {
    ($($name:ident => $op:ident,)*) => {
        $(
            /// Dart `_assert*` for a unary operation.
            pub fn $name(&self, expected: Option<DartObjectImpl>, operand: DartObjectImpl) -> EvalResult<()> {
                let ts = self.ts();
                self.check(expected, || operand.$op(&ts))
            }
        )*
    };
}

impl T {
    /// Dart `setUp` (`FeatureSets.latestWithExperiments`).
    pub fn new() -> T {
        T {
            world: Box::new(build_world()),
            feature_set: FeatureSet::new([Arc::from("patterns")]),
        }
    }

    /// Dart `_featureSet = FeatureSets.language_2_19`.
    pub fn set_language_2_19(&mut self) {
        self.feature_set = FeatureSet::new([]);
    }

    pub fn ts(&self) -> TestTypeSystem<'_> {
        TestTypeSystem { world: &self.world }
    }

    pub fn ctx(&self) -> Ctx<'_> {
        self.world.ctx()
    }

    pub fn tp(&self) -> &TypeProvider {
        &self.world.tp
    }

    /// Dart `expect(actual, expected)` on `DartObjectImpl`s.
    pub fn assert_eq(&self, expected: &DartObjectImpl, actual: &DartObjectImpl) {
        let ts = self.ts();
        assert!(
            actual.dart_eq(expected, &ts),
            "expected {}, actual {}",
            expected.display(&ts),
            actual.display(&ts)
        );
    }

    /// Dart `a == b` on `DartObjectImpl`s.
    pub fn eq(&self, a: &DartObjectImpl, b: &DartObjectImpl) -> bool {
        a.dart_eq(b, &self.ts())
    }

    /// Dart `_assert`.
    fn check(
        &self,
        expected: Option<DartObjectImpl>,
        f: impl FnOnce() -> EvalResult<DartObjectImpl>,
    ) -> EvalResult<()> {
        match expected {
            None => {
                let result = f();
                assert!(
                    result.is_err(),
                    "expected an EvaluationException, got {}",
                    result.unwrap().display(&self.ts())
                );
                Ok(())
            }
            Some(expected) => {
                let result = f()?;
                self.assert_eq(&expected, &result);
                Ok(())
            }
        }
    }

    binary_asserts! {
        assert_add => add,
        assert_concatenate => concatenate,
        assert_divide => divide,
        assert_eager_and => eager_and,
        assert_eager_or => eager_or,
        assert_eager_xor => eager_xor,
        assert_greater_than => greater_than,
        assert_greater_than_or_equal => greater_than_or_equal,
        assert_integer_divide => integer_divide,
        assert_less_than => less_than,
        assert_less_than_or_equal => less_than_or_equal,
        assert_logical_shift_right => logical_shift_right,
        assert_minus => minus,
        assert_remainder => remainder,
        assert_shift_left => shift_left,
        assert_shift_right => shift_right,
        assert_times => times,
    }

    unary_asserts! {
        assert_bit_not => bit_not,
        assert_logical_not => logical_not,
        assert_negated => negated,
        assert_perform_to_string => perform_to_string,
        assert_string_length => string_length,
    }

    pub fn assert_equal_equal(
        &self,
        expected: Option<DartObjectImpl>,
        left: DartObjectImpl,
        right: DartObjectImpl,
    ) -> EvalResult<()> {
        let ts = self.ts();
        self.check(expected, || {
            left.equal_equal(&ts, &self.feature_set, &right)
        })
    }

    pub fn assert_not_equal(
        &self,
        expected: Option<DartObjectImpl>,
        left: DartObjectImpl,
        right: DartObjectImpl,
    ) -> EvalResult<()> {
        let ts = self.ts();
        self.check(expected, || left.not_equal(&ts, &self.feature_set, &right))
    }

    pub fn assert_lazy_and(
        &self,
        expected: Option<DartObjectImpl>,
        left: DartObjectImpl,
        right: DartObjectImpl,
    ) -> EvalResult<()> {
        let ts = self.ts();
        self.check(expected, || left.lazy_and(&ts, || Ok(right)))
    }

    pub fn assert_lazy_or(
        &self,
        expected: Option<DartObjectImpl>,
        left: DartObjectImpl,
        right: DartObjectImpl,
    ) -> EvalResult<()> {
        let ts = self.ts();
        self.check(expected, || left.lazy_or(&ts, || Ok(right)))
    }

    /// Dart `_assertIdentical`.
    pub fn assert_identical(
        &self,
        expected: DartObjectImpl,
        left: DartObjectImpl,
        right: DartObjectImpl,
    ) {
        let result = left.is_identical2(&self.ts(), &right);
        self.assert_eq(&expected, &result);
    }

    fn object(&self, ty: TypeId, state: InstanceState) -> DartObjectImpl {
        DartObjectImpl::new(&self.ts(), ty, state)
    }

    pub fn bool_value(&self, value: Option<bool>) -> DartObjectImpl {
        let state = match value {
            None => BoolState::UNKNOWN_VALUE,
            Some(false) => BoolState::FALSE_STATE,
            Some(true) => BoolState::TRUE_STATE,
        };
        self.object(self.tp().bool_type(), InstanceState::Bool(state))
    }

    pub fn double_value(&self, value: Option<f64>) -> DartObjectImpl {
        self.object(
            self.tp().double_type(),
            InstanceState::Double(DoubleState::new(value)),
        )
    }

    pub fn int_value(&self, value: Option<i64>) -> DartObjectImpl {
        self.object(
            self.tp().int_type(),
            InstanceState::Int(IntState::new(value)),
        )
    }

    pub fn list_value(
        &self,
        element_type: TypeId,
        elements: Vec<DartObjectImpl>,
    ) -> DartObjectImpl {
        let ts = self.ts();
        let ty = self.tp().list_type(&self.ctx(), element_type);
        self.object(
            ty,
            InstanceState::List(Arc::new(ListState::new(&ts, element_type, elements, false))),
        )
    }

    pub fn map_value(
        &self,
        key_type: TypeId,
        value_type: TypeId,
        key_value_pairs: Vec<DartObjectImpl>,
    ) -> DartObjectImpl {
        let ts = self.ts();
        let mut map = DartObjectMap::new();
        let mut pairs = key_value_pairs.into_iter();
        while let (Some(k), Some(v)) = (pairs.next(), pairs.next()) {
            map.insert(&ts, k, v);
        }
        let ty = self.tp().map_type(&self.ctx(), key_type, value_type);
        self.object(
            ty,
            InstanceState::Map(Arc::new(MapState::new(
                &ts, key_type, value_type, map, false,
            ))),
        )
    }

    pub fn null_value(&self) -> DartObjectImpl {
        self.object(
            self.tp().null_type(),
            InstanceState::Null(NullState::NULL_STATE),
        )
    }

    pub fn record_value(
        &self,
        positional_fields: Vec<DartObjectImpl>,
        named_fields: Vec<(&str, DartObjectImpl)>,
    ) -> DartObjectImpl {
        let named: FieldMap = named_fields
            .into_iter()
            .map(|(k, v)| (Arc::from(k), v))
            .collect();
        self.object(
            self.tp().record_type(),
            InstanceState::Record(Arc::new(RecordState::new(positional_fields, named))),
        )
    }

    pub fn set_value(
        &self,
        element_type: TypeId,
        elements: Option<Vec<DartObjectImpl>>,
    ) -> DartObjectImpl {
        let ts = self.ts();
        let mut set = DartObjectSet::new();
        for e in elements.unwrap_or_default() {
            set.add(&ts, e);
        }
        let ty = self.tp().set_type(&self.ctx(), element_type);
        self.object(
            ty,
            InstanceState::Set(Arc::new(SetState::new(&ts, element_type, set, false))),
        )
    }

    pub fn string_value(&self, value: Option<&str>) -> DartObjectImpl {
        let state = match value {
            None => StringState::UNKNOWN_VALUE,
            Some(v) => StringState::new(v),
        };
        self.object(self.tp().string_type(), InstanceState::String(state))
    }

    pub fn symbol_value(&self, value: &str) -> DartObjectImpl {
        self.object(
            self.tp().symbol_type(),
            InstanceState::Symbol(SymbolState::new(Some(Arc::from(value)))),
        )
    }

    pub fn type_value(&self, value: TypeId) -> DartObjectImpl {
        let ts = self.ts();
        self.object(
            self.tp().type_type(),
            InstanceState::Type(TypeState::new(&ts, Some(value))),
        )
    }

    /// Dart `typeSystem.makeNullable(type)` for interface and function types.
    pub fn make_nullable(&self, t: TypeId) -> TypeId {
        let ctx = self.ctx();
        match *ctx.ty(t) {
            TypeKind::Interface {
                element,
                args,
                alias,
                ..
            } => ctx.intern(TypeKind::Interface {
                element,
                args,
                nullability: Nullability::Question,
                alias,
            }),
            TypeKind::Function(f) => ctx.intern(TypeKind::Function(FunctionTypeData {
                nullability: Nullability::Question,
                ..f
            })),
            _ => panic!("not needed by the tests"),
        }
    }

    /// `FutureOr<arg>`.
    pub fn future_or(&self, arg: TypeId) -> TypeId {
        self.tp().future_or_type(&self.ctx(), arg)
    }
}

//! The `elements` dump of a small element model, built by hand as the linker
//! would build it for:
//!
//! ```dart
//! class C {
//!   var x;
//!   C(this.x);
//! }
//! void f([dynamic a = 0]) {}
//! ```

use std::sync::Arc;

use dartr_ast::NodeId;
use dartr_element::{
    ClassElement, ClassFragment, ConstExprId, ConstructorElement, ConstructorFragment, Ctx, EId,
    ElementData, ElementFlags, ElementId, ExecutableElementData, ExecutableFragmentData, FId,
    FeatureSet, FieldElement, FieldFragment, FormalParameterElement, FormalParameterFragment,
    FragmentData, FragmentFlags, FragmentId, Generation, GetterElement, GetterFragment,
    InterfaceElementData, InterfaceFragmentData, LibraryElement, LibraryFragment,
    LibraryLanguageVersion, Metadata, Namespace, NoopSink, Nullability, OnceSlot, ParameterKind,
    PropertyAccessorElementData, PropertyAccessorFragmentData, PropertyInducingElementData,
    PropertyInducingFragmentData, SetterElement, SetterFragment, SourceRef, StoreId, Tag,
    TopLevelFunctionElement, TopLevelFunctionFragment, TypeId, TypeKind, TypeProvider, VarSlot,
    VariableElementData, VariableFragmentData, Version, WorldSnapshot,
};
use dartr_link::dump::{DumpSources, error_json, library_json};

/// The source of every const expression is `0` (the only one is the default
/// value of `a`).
struct Sources;

impl DumpSources for Sources {
    fn const_expr_source(&self, _store: StoreId, _expr: ConstExprId) -> String {
        "0".to_owned()
    }
}

fn fragment(
    names: &dartr_element::NamePool,
    name: Option<&str>,
    offset: Option<u32>,
    enclosing: FragmentId,
) -> FragmentData {
    let mut f = FragmentData::new(name.map(|n| names.intern(n)), offset);
    f.enclosing_fragment = Some(enclosing);
    f
}

fn element(
    names: &dartr_element::NamePool,
    name: &str,
    first: FragmentId,
    library: EId<LibraryElement>,
    enclosing: ElementId,
) -> ElementData {
    let mut e = ElementData::new(Some(names.intern(name)), first);
    e.library = Some(library);
    e.enclosing = Some(enclosing);
    e
}

fn formal_parameter(
    store: &dartr_element::ElementStore,
    names: &dartr_element::NamePool,
    tag: Tag,
    data: (&str, Option<u32>, ParameterKind),
    enclosing: (FragmentId, ElementId),
    library: EId<LibraryElement>,
) -> EId<FormalParameterElement> {
    let (name, offset, kind) = data;
    let f = FormalParameterFragment {
        variable: VariableFragmentData::new(fragment(names, Some(name), offset, enclosing.0)),
        parameter_kind: kind,
        private_name: None,
    };
    f.variable.fragment.flags.set(
        FragmentFlags::VARIABLE_FRAGMENT_HAS_IMPLICIT_TYPE,
        tag == Tag::FieldFormalParameter,
    );
    let f: FId<FormalParameterFragment> = store.add_fragment(f);
    // Field formal parameters share the data and vector of formal parameters
    // and differ by tag.
    let f = FId::<FormalParameterFragment>::from_raw(FragmentId::new(f.store(), tag, f.index()));
    let p: EId<FormalParameterElement> = store.add(FormalParameterElement {
        variable: VariableElementData::new(element(names, name, f.raw(), library, enclosing.1)),
        kind,
        type_: VarSlot::with(TypeId::DYNAMIC),
        base_formal_parameter: None,
        field: VarSlot::new(),
    });
    let p = EId::<FormalParameterElement>::from_raw(ElementId::new(p.store(), tag, p.index()));
    store.fragment(f).element.set_once(p.raw());
    p
}

#[test]
fn library_json_of_class_with_field_accessors_constructor_and_function() {
    let generation = Arc::new(Generation::new(0));
    let names = &generation.names;
    let mut store = generation.new_cycle_store();

    // The library and its only unit.
    let unit_id = FId::<LibraryFragment>::from_raw(FragmentId::new(store.id, Tag::Library, 0));
    let library: EId<LibraryElement> = store.add(LibraryElement {
        element: ElementData::new(Some(names.intern("")), unit_id.raw()),
        metadata: Metadata::default(),
        documentation_comment: None,
        language_version: LibraryLanguageVersion {
            package: Version {
                major: 3,
                minor: 13,
            },
            override_: Some(Version { major: 3, minor: 9 }),
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
    let unit = store.add_fragment::<LibraryFragment>(LibraryFragment::new(
        FragmentData::new(None, None),
        SourceRef {
            path: Arc::from("/p/lib/a.dart"),
            uri: Arc::from("package:p/a.dart"),
        },
        library,
    ));
    assert_eq!(unit, unit_id);
    store.fragment(unit).element.set_once(library.raw());

    // class C
    let class_f = store.add_fragment::<ClassFragment>(ClassFragment {
        interface: InterfaceFragmentData::new(fragment(names, Some("C"), Some(6), unit.raw())),
    });
    let class: EId<ClassElement> = store.add(ClassElement {
        interface: InterfaceElementData::new(element(
            names,
            "C",
            class_f.raw(),
            library,
            library.raw(),
        )),
    });
    store.fragment(class_f).element.set_once(class.raw());

    // var x;
    let field_f = store.add_fragment::<FieldFragment>(FieldFragment {
        property: PropertyInducingFragmentData {
            variable: VariableFragmentData::new(fragment(
                names,
                Some("x"),
                Some(16),
                class_f.raw(),
            )),
            induced_getter: None,
            induced_setter: None,
        },
        inherits_covariant: Default::default(),
    });
    let field: EId<FieldElement> = store.add(FieldElement {
        property: PropertyInducingElementData::new(element(
            names,
            "x",
            field_f.raw(),
            library,
            class.raw(),
        )),
    });
    store.fragment(field_f).element.set_once(field.raw());
    let field_flags = &store.fragment(field_f).flags;
    field_flags.set(FragmentFlags::VARIABLE_FRAGMENT_HAS_IMPLICIT_TYPE, true);
    field_flags.set(
        FragmentFlags::PROPERTY_INDUCING_FRAGMENT_IS_ORIGIN_DECLARATION,
        true,
    );
    store.get(field).type_.set(Some(TypeId::DYNAMIC));

    // The synthetic getter `x` and setter `x=` of the field.
    let accessor_fragment = |name_offset: Option<u32>| PropertyAccessorFragmentData {
        executable: ExecutableFragmentData::new(fragment(
            names,
            Some("x"),
            name_offset,
            class_f.raw(),
        )),
        inducing_variable: Some(field_f.upcast()),
    };
    let accessor_element = |first: FragmentId| PropertyAccessorElementData {
        executable: ExecutableElementData::new(element(names, "x", first, library, class.raw())),
        variable: VarSlot::with(field.upcast()),
    };
    let getter_f = store.add_fragment::<GetterFragment>(GetterFragment {
        accessor: accessor_fragment(None),
    });
    let getter: EId<GetterElement> = store.add(GetterElement {
        accessor: accessor_element(getter_f.raw()),
    });
    store.fragment(getter_f).element.set_once(getter.raw());
    let setter_f = store.add_fragment::<SetterFragment>(SetterFragment {
        accessor: accessor_fragment(None),
    });
    let setter: EId<SetterElement> = store.add(SetterElement {
        accessor: accessor_element(setter_f.raw()),
    });
    store.fragment(setter_f).element.set_once(setter.raw());
    for f in [getter_f.raw(), setter_f.raw()] {
        let flags = &store.fragment_data(f).unwrap().flags;
        flags.set(
            FragmentFlags::PROPERTY_ACCESSOR_FRAGMENT_IS_ORIGIN_VARIABLE,
            true,
        );
    }
    // The parameter of the setter has no implicit type flag: its "inf" is the
    // one of the field.
    let value = formal_parameter(
        &store,
        names,
        Tag::FormalParameter,
        ("_x", None, ParameterKind::Required),
        (setter_f.raw(), setter.raw()),
        library,
    );
    store.get(setter).return_type.set(Some(TypeId::VOID));

    // C(this.x);
    let ctor_f = store.add_fragment::<ConstructorFragment>(ConstructorFragment {
        executable: ExecutableFragmentData::new(fragment(names, Some("new"), None, class_f.raw())),
        constant_initializers: OnceSlot::new(),
        new_keyword_offset: None,
        factory_keyword_offset: None,
        type_name: Some(names.intern("C")),
        type_name_offset: Some(21),
        period_offset: None,
        name_end: None,
        this_keyword_offset: None,
    });
    let ctor: EId<ConstructorElement> = store.add(ConstructorElement {
        executable: ExecutableElementData::new(element(
            names,
            "new",
            ctor_f.raw(),
            library,
            class.raw(),
        )),
        redirected_constructor: VarSlot::new(),
        super_constructor: VarSlot::new(),
    });
    store.fragment(ctor_f).element.set_once(ctor.raw());
    let this_x = formal_parameter(
        &store,
        names,
        Tag::FieldFormalParameter,
        ("x", Some(28), ParameterKind::Required),
        (ctor_f.raw(), ctor.raw()),
        library,
    );
    store.get(this_x).field.set(Some(field));

    // void f([dynamic a = 0]) {}
    let function_f = store.add_fragment::<TopLevelFunctionFragment>(TopLevelFunctionFragment {
        executable: ExecutableFragmentData::new(fragment(names, Some("f"), Some(40), unit.raw())),
    });
    let function: EId<TopLevelFunctionElement> = store.add(TopLevelFunctionElement {
        executable: ExecutableElementData::new(element(
            names,
            "f",
            function_f.raw(),
            library,
            library.raw(),
        )),
    });
    store.fragment(function_f).element.set_once(function.raw());
    store.get(function).return_type.set(Some(TypeId::VOID));
    let a = formal_parameter(
        &store,
        names,
        Tag::FormalParameter,
        ("a", Some(51), ParameterKind::Positional),
        (function_f.raw(), function.raw()),
        library,
    );
    let a_fragment = store.get(a).first_fragment();
    store.fragment_mut(a_fragment).variable.constant_initializer =
        Some(ConstExprId(NodeId::from_index(0)));

    // Structure (builder phase).
    store.get_mut(class).fields.push(field);
    store.get_mut(class).getters.push(getter);
    store.get_mut(class).setters.push(setter);
    store.get_mut(class).constructors.push(ctor);
    store
        .get_mut(class)
        .flags
        .set(ElementFlags::INSTANCE_ELEMENT_IS_SIMPLY_BOUNDED, true);
    store.get_mut(class).has_non_final_field.set(true);
    store.get_mut(field).property.getter = Some(getter);
    store.get_mut(field).property.setter = Some(setter);
    store.get_mut(setter).formal_params.push(value);
    store.get_mut(ctor).formal_params.push(this_x);
    store.get_mut(function).formal_params.push(a);
    store.get_mut(library).classes.push(class);
    store.get_mut(library).top_level_functions.push(function);
    store.fragment_mut(unit).classes.push(class_f);
    store.fragment_mut(unit).functions.push(function_f);
    // Insertion order f, C, dynamic: the dump sorts by name.
    let mut namespace = Namespace::default();
    namespace
        .defined_names
        .insert(names.intern("f"), function.raw());
    namespace
        .defined_names
        .insert(names.intern("dynamic"), ElementId::DYNAMIC);
    namespace
        .defined_names
        .insert(names.intern("C"), class.raw());
    store
        .get(library)
        .export_namespace
        .set_once(Arc::new(namespace));

    let world = WorldSnapshot::new(generation.clone());
    let tp = TypeProvider::default();
    let features = FeatureSet::default();
    let ctx = Ctx {
        world: &world,
        current: Some(&store),
        local: None,
        tp: &tp,
        features: &features,
        req: &NoopSink,
    };
    // Field type `C?` (set after the structure, as type resolution does).
    let c_nullable = ctx.intern(TypeKind::Interface {
        element: class.upcast(),
        args: Default::default(),
        nullability: Nullability::Question,
        alias: None,
    });
    ctx.get(field).type_.set(Some(c_nullable));

    let line = library_json(&ctx, &Sources, "/p/lib/a.dart", library);
    let expected = concat!(
        r#"{"path":"/p/lib/a.dart","uri":"package:p/a.dart","lang":"3.9","#,
        r#""units":[{"path":"/p/lib/a.dart"}],"imports":[],"exports":[],"#,
        r#""exportNamespace":{"C":"package:p/a.dart::C","dynamic":"<no-library>::dynamic","f":"package:p/a.dart::f"},"#,
        r#""elements":["#,
        r#"{"k":"class","n":"C","u":0,"o":6,"f":["hasNonFinalField","simplyBounded"],"tp":[],"super":null,"mixins":[],"interfaces":[],"members":["#,
        r#"{"k":"field","n":"x","o":16,"f":[],"type":"C?","inf":true,"typeInferenceError":null},"#,
        r#"{"k":"getter","n":"x","f":["originVariable","simplyBounded"],"type":"C? Function()","inf":true,"var":"package:p/a.dart::C.x"},"#,
        r#"{"k":"setter","n":"x","f":["originVariable","simplyBounded"],"type":"void Function(dynamic)","inf":true,"var":"package:p/a.dart::C.x","#,
        r#""params":[{"n":"_x","kind":"requiredPositional","type":"dynamic","inf":true,"f":[],"default":null}]},"#,
        r#"{"k":"ctor","n":"new","f":["simplyBounded"],"type":"C Function(dynamic)","inf":true,"#,
        r#""params":[{"n":"x","kind":"requiredPositional","type":"dynamic","inf":true,"f":["fieldFormal","final"],"default":null}],"#,
        r#""redirected":null,"superCtor":null}]},"#,
        r#"{"k":"function","n":"f","u":0,"o":40,"f":["simplyBounded"],"type":"void Function([dynamic])","inf":false,"tp":[],"#,
        r#""params":[{"n":"a","kind":"optionalPositional","type":"dynamic","inf":false,"f":["hasDefaultValue"],"default":"0"}]}]}"#,
    );
    assert_eq!(line, expected);
    serde_json::from_str::<serde_json::Value>(&line).expect("valid JSON");
}

#[test]
fn error_json_escapes_like_dart_json_encode() {
    assert_eq!(
        error_json("/p/a \"b\"\n.dart", "NotLibraryButPartResult"),
        r#"{"path":"/p/a \"b\"\n.dart","error":"NotLibraryButPartResult"}"#,
    );
}

// Dart source: pkg/analyzer/lib/src/dart/resolver/flow_analysis_visitor.dart
// (TypeSystemOperations.isFinal, isVariableFinal, variableType,
// isPropertyPromotable, whyPropertyIsNotPromotable) and the element getters
// they read (pkg/analyzer/lib/src/dart/element/element.dart).

//! The element getters of `TypeSystemOperations` that flow analysis uses.
//! Classes are built from Dart source by `support::SourceTest`; the flags
//! that the linker computes (`isFinal`, `isExternal`, `isPromotable`) are
//! set on the field fragments as the linker sets them, because the source
//! builder of the tests is not a linker.

mod support;

use dartr_element::{
    EId, ElemRef, ElementData, ElementId, FId, FieldElement, FormalParameterElement,
    FormalParameterFragment, FragmentData, FragmentFlags, FragmentId, LocalVariableElement,
    LocalVariableFragment, ParameterKind, PromotableElement, Tag, TypeId, VarSlot,
    VariableElementData, VariableFragmentData,
};
use dartr_flow::flow_analysis_operations::{
    FlowAnalysisOperations, PropertyNonPromotabilityReason,
};
use dartr_flow::type_analyzer_operations::TypeAnalyzerOperations;
use dartr_typesystem::member;
use dartr_typesystem::type_algebra::MapSubstitution;
use support::SourceTest;

const SOURCE: &str = r#"
class A<T> {
  final int? _promotable = null;
  final int? publicField = null;
  int? _notFinal;
  final int? _external = null;
  final int? _conflict = null;
  final T? _generic = null;
  int? get _getter => null;
  int? _method() => null;
}
"#;

/// Sets [flag] on the first fragment of the field [name] of `A`.
fn set_field_flag(test: &SourceTest, name: &str, flag: FragmentFlags) {
    let ctx = test.ctx();
    let class = test.class_or_mixin("A");
    let field: EId<FieldElement> = *ctx
        .instance(class.upcast())
        .fields
        .iter()
        .find(|f| ctx.name_str(ctx.get(**f).name.unwrap()) == name)
        .unwrap_or_else(|| panic!("no field A.{name}"));
    let first = ctx.element_data(field.raw()).unwrap().first_fragment;
    ctx.fragment_data(first).unwrap().flags.set(flag, true);
}

/// The class `A` of [SOURCE], with the flags of the linker: `final` fields
/// are final, `_external` is external, `_promotable` and `_generic` are
/// promotable (`_conflict` is not, as when another declaration of the
/// library has the same name and prevents the promotion).
fn build() -> SourceTest {
    let test = SourceTest::new(SOURCE);
    for name in [
        "_promotable",
        "publicField",
        "_external",
        "_conflict",
        "_generic",
    ] {
        set_field_flag(&test, name, FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL);
    }
    set_field_flag(&test, "_external", FragmentFlags::VARIABLE_FRAGMENT_IS_EXTERNAL);
    for name in ["_promotable", "_generic"] {
        set_field_flag(&test, name, FragmentFlags::FIELD_FRAGMENT_IS_PROMOTABLE);
    }
    test
}

#[test]
fn property_promotability_of_getters_and_methods() {
    let test = build();
    let ops = test.t.type_system_operations();
    let cases: [(ElemRef, Option<PropertyNonPromotabilityReason>, bool); 7] = [
        (test.getter("A", "_promotable"), None, true),
        (
            test.getter("A", "publicField"),
            Some(PropertyNonPromotabilityReason::IsNotPrivate),
            false,
        ),
        (
            test.getter("A", "_notFinal"),
            Some(PropertyNonPromotabilityReason::IsNotFinal),
            false,
        ),
        (
            test.getter("A", "_external"),
            Some(PropertyNonPromotabilityReason::IsExternal),
            false,
        ),
        // Not promotable for a reason outside the field (a conflict).
        (test.getter("A", "_conflict"), None, false),
        // A declared getter: its field is synthetic.
        (
            test.getter("A", "_getter"),
            Some(PropertyNonPromotabilityReason::IsNotField),
            false,
        ),
        // A method tear-off.
        (
            test.method("A", "_method"),
            Some(PropertyNonPromotabilityReason::IsNotField),
            false,
        ),
    ];
    for (property, reason, promotable) in cases {
        let name = member::name(&test.ctx(), property).unwrap();
        assert_eq!(
            ops.why_property_is_not_promotable(&property),
            reason,
            "whyPropertyIsNotPromotable({name})"
        );
        assert_eq!(
            ops.is_property_promotable(&property),
            promotable,
            "isPropertyPromotable({name})"
        );
    }
}

#[test]
fn property_promotability_of_substituted_getter() {
    let test = build();
    let ctx = test.ctx();
    let ops = test.t.type_system_operations();
    let t = test.type_parameter("T");
    let substitution = MapSubstitution::from_pairs(&[t], &[ctx.tp.int_type()]);
    let generic = member::substitute(&ctx, test.getter("A", "_generic"), &substitution);
    let not_final = member::substitute(&ctx, test.getter("A", "_notFinal"), &substitution);
    assert!(matches!(generic, ElemRef::Member(_)));
    assert!(matches!(not_final, ElemRef::Member(_)));

    assert!(ops.is_property_promotable(&generic));
    assert_eq!(ops.why_property_is_not_promotable(&generic), None);
    assert!(!ops.is_property_promotable(&not_final));
    assert_eq!(
        ops.why_property_is_not_promotable(&not_final),
        Some(PropertyNonPromotabilityReason::IsNotFinal)
    );
}

/// Adds a local variable `name` (Dart `LocalVariableElementImpl`) with the
/// fragment flags [flags] and the type [ty] (not set when `None`).
fn local_variable(
    test: &SourceTest,
    name: &str,
    flags: &[FragmentFlags],
    ty: Option<TypeId>,
) -> EId<PromotableElement> {
    let ctx = test.ctx();
    let store = &test.t.store;
    let fd = FragmentData::new(Some(ctx.name(name)), Some(0));
    for &flag in flags {
        fd.flags.set(flag, true);
    }
    let fragment: FId<LocalVariableFragment> = store.add_fragment(LocalVariableFragment {
        variable: VariableFragmentData::new(fd),
        pattern: Default::default(),
    });
    let element: EId<LocalVariableElement> = store.add(LocalVariableElement {
        variable: VariableElementData::new(ElementData::new(
            Some(ctx.name(name)),
            fragment.raw(),
        )),
        type_: match ty {
            Some(ty) => VarSlot::with(ty),
            None => VarSlot::new(),
        },
    });
    store.fragment(fragment).element.set_once(element.raw());
    element.upcast()
}

/// Adds a formal parameter `name` with the id tag [tag]
/// (`FormalParameter`, `FieldFormalParameter`, `SuperFormalParameter`).
fn formal_parameter(
    test: &SourceTest,
    name: &str,
    tag: Tag,
    flags: &[FragmentFlags],
    ty: TypeId,
) -> EId<PromotableElement> {
    let ctx = test.ctx();
    let store = &test.t.store;
    let fd = FragmentData::new(Some(ctx.name(name)), Some(0));
    for &flag in flags {
        fd.flags.set(flag, true);
    }
    let fragment: FId<FormalParameterFragment> = store.add_fragment(FormalParameterFragment {
        variable: VariableFragmentData::new(fd),
        parameter_kind: ParameterKind::Required,
        private_name: None,
    });
    // Field and super formal parameters share the data of formal parameters
    // and differ by tag.
    let fragment = FragmentId::new(fragment.store(), tag, fragment.index());
    let element: EId<FormalParameterElement> = store.add(FormalParameterElement {
        variable: VariableElementData::new(ElementData::new(Some(ctx.name(name)), fragment)),
        kind: ParameterKind::Required,
        type_: VarSlot::with(ty),
        base_formal_parameter: None,
        field: VarSlot::new(),
    });
    let element = ElementId::new(element.store(), tag, element.index());
    ctx.fragment_data(fragment).unwrap().element.set_once(element);
    element.cast::<PromotableElement>().unwrap()
}

#[test]
fn is_final_of_local_variables_and_parameters() {
    let test = SourceTest::new("");
    let ctx = test.ctx();
    let ops = test.t.type_system_operations();
    let int = ctx.tp.int_type();
    let cases = [
        (local_variable(&test, "v", &[], Some(int)), false),
        (
            local_variable(
                &test,
                "f",
                &[FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL],
                Some(int),
            ),
            true,
        ),
        // Dart `isFinal` is false for `const` variables.
        (
            local_variable(
                &test,
                "c",
                &[FragmentFlags::VARIABLE_FRAGMENT_IS_CONST],
                Some(int),
            ),
            false,
        ),
        (formal_parameter(&test, "p", Tag::FormalParameter, &[], int), false),
        (
            formal_parameter(
                &test,
                "fp",
                Tag::FormalParameter,
                &[FragmentFlags::VARIABLE_FRAGMENT_IS_FINAL],
                int,
            ),
            true,
        ),
        // `this.x` and `super.x` parameters are always final.
        (formal_parameter(&test, "x", Tag::FieldFormalParameter, &[], int), true),
        (formal_parameter(&test, "y", Tag::SuperFormalParameter, &[], int), true),
    ];
    for (variable, expected) in cases {
        let name = ctx.name_str(ctx.element_data(variable.raw()).unwrap().name.unwrap());
        assert_eq!(FlowAnalysisOperations::is_final(&ops, variable), expected, "isFinal({name})");
        assert_eq!(ops.is_variable_final(variable), expected, "isVariableFinal({name})");
    }
}

#[test]
fn variable_type_of_local_variables_and_parameters() {
    let test = SourceTest::new("");
    let ctx = test.ctx();
    let ops = test.t.type_system_operations();
    let int_q = test.t.parse_type("int?");
    let string = ctx.tp.string_type();

    let local = local_variable(&test, "v", &[], Some(int_q));
    let parameter = formal_parameter(&test, "p", Tag::FieldFormalParameter, &[], string);
    // Before the resolver sets the type of an implicitly typed variable.
    let untyped = local_variable(&test, "u", &[], None);

    assert_eq!(ops.variable_type(local).unwrap_type_view(), int_q);
    assert_eq!(ops.variable_type(parameter).unwrap_type_view(), string);
    assert_eq!(ops.variable_type(untyped).unwrap_type_view(), TypeId::INVALID);
}

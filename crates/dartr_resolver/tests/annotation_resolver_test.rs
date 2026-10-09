//! Representative ports of
//! `pkg/analyzer/test/src/dart/resolution/metadata_test.dart`.

mod support;

use dartr_ast::NodeKind;
use dartr_element::{ElemRef, FormalParameterElement, Tag};
use dartr_typesystem::member;
use support::{Analyzed, analyze};

fn run(files: &[(&str, &str)]) -> Option<Analyzed> {
    let result = analyze(files);
    if result.is_none() {
        eprintln!("skipped: no Dart SDK on PATH");
    }
    result
}

fn annotation_element(a: &Analyzed, search: &str) -> ElemRef {
    let node = a.node_at(NodeKind::Annotation, search, 0, 0);
    *a.unit()
        .tables
        .element
        .get(node)
        .expect("annotation element")
}

fn base(a: &Analyzed, element: ElemRef) -> dartr_element::ElementId {
    member::base_element(&a.ctx(a.unit()), element)
}

#[test]
fn class_unnamed_constructor_and_argument_parameter() {
    let source = r#"
class A {
  final int value;
  const A(this.value);
}

@A(42)
void f() {}
"#;
    let Some(a) = run(&[("main.dart", source)]) else {
        return;
    };

    let constructor = annotation_element(&a, "@A(42)");
    assert_eq!(base(&a, constructor).tag(), Tag::Constructor);
    assert_eq!(
        a.element_name(base(&a, constructor)).as_deref(),
        Some("new")
    );

    let argument = a.node_at(NodeKind::IntegerLiteral, "42", 0, 0);
    let parameter = *a
        .unit()
        .tables
        .param_element
        .get(argument)
        .expect("corresponding parameter");
    assert!(base(&a, parameter).is::<FormalParameterElement>());
    assert_eq!(
        a.element_name(base(&a, parameter)).as_deref(),
        Some("value")
    );
    assert!(
        a.diagnostic_names().is_empty(),
        "{:?}",
        a.diagnostic_names()
    );
}

#[test]
fn named_constructor_is_on_prefixed_annotation_name() {
    let source = r#"
class A {
  final int value;
  const A.named(this.value);
}

@A.named(42)
void f() {}
"#;
    let Some(a) = run(&[("main.dart", source)]) else {
        return;
    };

    let constructor = annotation_element(&a, "@A.named");
    assert_eq!(
        a.element_name(base(&a, constructor)).as_deref(),
        Some("named")
    );
    let identifier = a.node_at(NodeKind::SimpleIdentifier, "named(42)", 0, 0);
    assert_eq!(a.element_of(identifier), Some(base(&a, constructor)));
}

#[test]
fn static_const_fields_local_and_imported() {
    let main = r#"
import 'values.dart' as values;

class Local {
  static const marker = 1;
}

@Local.marker
void local() {}

@values.marker
void imported() {}
"#;
    let values = "const marker = 2;";
    let Some(a) = run(&[("main.dart", main), ("values.dart", values)]) else {
        return;
    };

    let local = annotation_element(&a, "@Local.marker");
    let imported = annotation_element(&a, "@values.marker");
    assert_eq!(base(&a, local).tag(), Tag::Getter);
    assert_eq!(base(&a, imported).tag(), Tag::Getter);
    assert_eq!(a.element_name(base(&a, local)).as_deref(), Some("marker"));
    assert_eq!(
        a.element_name(base(&a, imported)).as_deref(),
        Some("marker")
    );
    assert!(
        a.diagnostic_names().is_empty(),
        "{:?}",
        a.diagnostic_names()
    );
}

#[test]
fn imported_prefix_class_named_constructor() {
    let main = r#"
import 'model.dart' as model;

@model.A.named(42)
void f() {}
"#;
    let model = r#"
class A {
  final int value;
  const A.named(this.value);
}
"#;
    let Some(a) = run(&[("main.dart", main), ("model.dart", model)]) else {
        return;
    };

    let constructor = annotation_element(&a, "@model.A.named");
    assert_eq!(
        a.element_name(base(&a, constructor)).as_deref(),
        Some("named")
    );
    let prefix = a.node_at(NodeKind::SimpleIdentifier, "model.A", 0, 0);
    assert_eq!(a.element_of(prefix).unwrap().tag(), Tag::Prefix);
    let class_name = a.node_at(NodeKind::SimpleIdentifier, "A.named", 0, 0);
    assert_eq!(a.element_of(class_name).unwrap().tag(), Tag::Class);
    let constructor_name = a.node_at(NodeKind::SimpleIdentifier, "named(42)", 0, 0);
    assert_eq!(a.element_of(constructor_name), Some(base(&a, constructor)));
}

#[test]
fn local_const_variable_is_a_valid_annotation() {
    let source = r#"
void f() {
  const marker = 1;
  @marker
  var value = 0;
}
"#;
    let Some(a) = run(&[("main.dart", source)]) else {
        return;
    };

    let name = a.node_at(NodeKind::SimpleIdentifier, "marker\n", 0, 0);
    let element = a.element_of(name).expect("local const element");
    assert_eq!(element.tag(), Tag::LocalVariable);
    assert!(
        a.diagnostic_names().is_empty(),
        "{:?}",
        a.diagnostic_names()
    );
}

#[test]
fn generic_constructor_infers_and_accepts_explicit_type_arguments() {
    let source = r#"
class A<T> {
  final T value;
  const A(this.value);
}

@A(42)
void inferred() {}

@A<String>('value')
void explicit() {}
"#;
    let Some(a) = run(&[("main.dart", source)]) else {
        return;
    };
    let ctx = a.ctx(a.unit());

    let inferred = annotation_element(&a, "@A(42)");
    let inferred_parameter = member::formal_parameters(&ctx, inferred)[0];
    assert_eq!(a.type_str(member::type_(&ctx, inferred_parameter)), "int");

    let explicit = annotation_element(&a, "@A<String>");
    let explicit_parameter = member::formal_parameters(&ctx, explicit)[0];
    assert_eq!(
        a.type_str(member::type_(&ctx, explicit_parameter)),
        "String"
    );
    assert!(
        a.diagnostic_names().is_empty(),
        "{:?}",
        a.diagnostic_names()
    );
}

#[test]
fn explicit_generic_annotation_context_types_empty_list() {
    let source = r#"
class A<T> {
  final List<T> values;
  const A(this.values);
}

@A<int>([])
void f() {}
"#;
    let Some(a) = run(&[("main.dart", source)]) else {
        return;
    };
    let ctx = a.ctx(a.unit());
    let list = a.node_at(NodeKind::ListLiteral, "[]", 0, 0);
    let list_type = *a
        .unit()
        .tables
        .static_type
        .get(list)
        .expect("list static type");
    assert_eq!(a.type_str(list_type), "List<int>");

    let parameter = *a
        .unit()
        .tables
        .param_element
        .get(list)
        .expect("list corresponding parameter");
    assert_eq!(a.type_str(member::type_(&ctx, parameter)), "List<int>");

    let constructor = annotation_element(&a, "@A<int>");
    let instantiated_parameter = member::formal_parameters(&ctx, constructor)[0];
    assert_eq!(
        a.type_str(member::type_(&ctx, instantiated_parameter)),
        "List<int>"
    );
    assert!(
        a.diagnostic_names().is_empty(),
        "{:?}",
        a.diagnostic_names()
    );
}

#[test]
fn inferred_generic_annotation_uses_list_element_type_for_class_and_alias() {
    let source = r#"
class A<T> {
  final List<T> values;
  const A(this.values);
}

typedef B<U> = A<U>;

@A([1])
void direct() {}

@B([2])
void alias() {}
"#;
    let Some(a) = run(&[("main.dart", source)]) else {
        return;
    };
    let ctx = a.ctx(a.unit());

    for (search, value) in [("@A([1])", "[1]"), ("@B([2])", "[2]")] {
        let list = a.node_at(NodeKind::ListLiteral, value, 0, 0);
        let list_type = *a
            .unit()
            .tables
            .static_type
            .get(list)
            .expect("list static type");
        assert_eq!(a.type_str(list_type), "List<int>", "{search}");

        let parameter = *a
            .unit()
            .tables
            .param_element
            .get(list)
            .expect("list corresponding parameter");
        assert_eq!(
            a.type_str(member::type_(&ctx, parameter)),
            "List<int>",
            "{search}"
        );

        let constructor = annotation_element(&a, search);
        let instantiated_parameter = member::formal_parameters(&ctx, constructor)[0];
        assert_eq!(
            a.type_str(member::type_(&ctx, instantiated_parameter)),
            "List<int>",
            "{search}"
        );
    }
    assert!(
        a.diagnostic_names().is_empty(),
        "{:?}",
        a.diagnostic_names()
    );
}

#[test]
fn generic_type_alias_constructor_infers_alias_parameter() {
    let source = r#"
class A<T> {
  final T value;
  const A.named(this.value);
}

typedef B<U> = A<U>;

@B.named(42)
void f() {}
"#;
    let Some(a) = run(&[("main.dart", source)]) else {
        return;
    };
    let ctx = a.ctx(a.unit());
    let constructor = annotation_element(&a, "@B.named");
    let parameter = member::formal_parameters(&ctx, constructor)[0];
    assert_eq!(a.type_str(member::type_(&ctx, parameter)), "int");
    assert_eq!(
        a.element_name(base(&a, constructor)).as_deref(),
        Some("named")
    );
}

#[test]
fn explicit_type_argument_must_match_bound() {
    let source = r#"
class A<T extends num> {
  final T value;
  const A(this.value);
}

@A<String>('value')
void f() {}
"#;
    let Some(a) = run(&[("main.dart", source)]) else {
        return;
    };

    assert!(
        a.diagnostic_names()
            .iter()
            .any(|name| name.starts_with("type_argument_not_matching_bounds@")),
        "{:?}",
        a.diagnostic_names()
    );
}

#[test]
fn invalid_and_undefined_annotations_report_at_annotation() {
    let source = r#"
var mutable = 0;

@mutable
void invalid() {}

@missing
void undefined() {}
"#;
    let Some(a) = run(&[("main.dart", source)]) else {
        return;
    };
    let diagnostics = a.diagnostic_names();
    assert!(
        diagnostics
            .iter()
            .any(|name| name.starts_with("invalid_annotation@")),
        "{diagnostics:?}"
    );
    assert!(
        diagnostics
            .iter()
            .any(|name| name.starts_with("undefined_annotation@")),
        "{diagnostics:?}"
    );
}

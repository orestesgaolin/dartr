//! Integration tests of units C4 and C5: property accesses, prefixed
//! identifiers, index expressions, assignments (also compound and `??=`),
//! binary, prefix and postfix expressions. Each test links a small library
//! against the SDK, runs the library analyzer and checks the static types,
//! elements, read / write elements and types, and diagnostics that the
//! Dart analyzer gives for the same code (`assertResolvedNodeText` of
//! `pkg/analyzer/test/src/dart/resolution/*_test.dart`).

mod support;

use dartr_ast::{NodeId, NodeKind};
use dartr_element::{ElemRef, ElementId, Tag};
use support::{Analyzed, analyze};

fn run(source: &str) -> Option<Analyzed> {
    let a = analyze(&[("main.dart", source)]);
    if a.is_none() {
        eprintln!("skipped: no Dart SDK on PATH");
    }
    a
}

/// The static type of the node of [kind] at the first occurrence of
/// [search], as a display string.
fn type_at(a: &Analyzed, kind: NodeKind, search: &str) -> String {
    let node = a.node_at(kind, search, 0, 0);
    type_of(a, node)
}

/// The longest node of [kind] that starts at the first occurrence of
/// [search].
fn outermost_at(a: &Analyzed, kind: NodeKind, search: &str) -> NodeId {
    let offset = a.source().find(search).expect("search") as u32;
    let ast = &a.unit().ast;
    (0..ast.node_count())
        .map(NodeId::from_index)
        .filter(|&n| ast.kind(n) == kind && ast.offset(n) == offset)
        .max_by_key(|&n| ast.end(n))
        .expect("node")
}

fn type_of(a: &Analyzed, node: NodeId) -> String {
    match a.unit().tables.static_type.get(node) {
        Some(&t) => a.type_str(t),
        None => "<none>".to_string(),
    }
}

/// `<enclosing name>.<name>` of [e] and its tag.
fn element_desc(a: &Analyzed, e: ElementId) -> (String, Tag) {
    let unit = a.unit();
    let ctx = a.ctx(unit);
    let enclosing = ctx
        .element_data(e)
        .and_then(|d| d.enclosing)
        .and_then(|p| a.element_name(p))
        .unwrap_or_default();
    (
        format!("{enclosing}.{}", a.element_name(e).unwrap_or_default()),
        e.tag(),
    )
}

fn base(a: &Analyzed, r: ElemRef) -> ElementId {
    match r {
        ElemRef::Base(e) => e,
        ElemRef::Member(m) => {
            let unit = a.unit();
            a.ctx(unit).member(m).base
        }
    }
}

#[test]
fn binary_operators_refine_numeric_types_and_resolve_operator_methods() {
    let source = r#"
void f(int a, int b, num c, double d) {
  a + b;
  a + c;
  a * d;
  a / b;
  a == b;
  a < b;
  -a;
  ~a;
  !(a < b);
}
"#;
    let Some(a) = run(source) else {
        return;
    };
    assert_eq!(type_at(&a, NodeKind::BinaryExpression, "a + b"), "int");
    assert_eq!(type_at(&a, NodeKind::BinaryExpression, "a + c"), "num");
    assert_eq!(type_at(&a, NodeKind::BinaryExpression, "a * d"), "double");
    assert_eq!(type_at(&a, NodeKind::BinaryExpression, "a / b"), "double");
    assert_eq!(type_at(&a, NodeKind::BinaryExpression, "a == b"), "bool");
    assert_eq!(type_at(&a, NodeKind::BinaryExpression, "a < b"), "bool");
    assert_eq!(type_at(&a, NodeKind::PrefixExpression, "-a"), "int");
    assert_eq!(type_at(&a, NodeKind::PrefixExpression, "~a"), "int");
    assert_eq!(type_at(&a, NodeKind::PrefixExpression, "!(a < b)"), "bool");

    // The operator element and the static invoke type of `a + b`.
    let plus = a.node_at(NodeKind::BinaryExpression, "a + b", 0, 0);
    let element = a.element_of(plus).expect("operator element");
    assert_eq!(
        element_desc(&a, element),
        ("num.+".to_string(), Tag::Method)
    );
    let invoke = *a.unit().tables.invoke_type.get(plus).expect("invoke type");
    assert_eq!(a.type_str(invoke), "num Function(num)");
    // `==` resolves to `num.==` with the left type promoted to non-null.
    let eq = a.node_at(NodeKind::BinaryExpression, "a == b", 0, 0);
    let element = a.element_of(eq).expect("== element");
    assert_eq!(element_desc(&a, element).0, "num.==");
    // `-a` is `int.unary-` (the element name is `-`, its lookup name
    // `unary-`).
    let neg = a.node_at(NodeKind::PrefixExpression, "-a", 0, 0);
    let element = a.element_of(neg).expect("unary- element");
    assert_eq!(
        element_desc(&a, element),
        ("int.-".to_string(), Tag::Method)
    );
    assert!(
        a.diagnostic_names().is_empty(),
        "{:?}",
        a.diagnostic_names()
    );
}

#[test]
fn logical_operators_promote_through_flow_analysis() {
    let source = r#"
void f(Object o, bool b) {
  o is int && o > 0;
  o is! String || o.length > 0;
  b ?? 1;
}
"#;
    let Some(a) = run(source) else {
        return;
    };
    assert_eq!(
        type_at(&a, NodeKind::BinaryExpression, "o is int &&"),
        "bool"
    );
    // `o` is promoted to `int` on the right of `&&`: `o > 0` is `int.>`.
    let gt = a.node_at(NodeKind::BinaryExpression, "o > 0", 0, 0);
    assert_eq!(type_of(&a, gt), "bool");
    let element = a.element_of(gt).expect("> element");
    assert_eq!(element_desc(&a, element).0, "num.>");
    // `o` is promoted to `String` on the right of `||`.
    assert_eq!(type_at(&a, NodeKind::PrefixedIdentifier, "o.length"), "int");
    // `b ?? 1`: UP(NonNull(bool), int).
    assert_eq!(type_at(&a, NodeKind::BinaryExpression, "b ?? 1"), "Object");
}

#[test]
fn compound_and_if_null_assignments_have_read_and_write_results() {
    let source = r#"
class A {
  int x = 0;
  num? y;
}

void f(A a, int i, double d, int? n) {
  i += 1;
  d -= i;
  a.x *= 2;
  a.y ??= 1.5;
  n ??= i;
  i = 3;
}
"#;
    let Some(a) = run(source) else {
        return;
    };
    let i_plus = a.node_at(NodeKind::AssignmentExpression, "i += 1", 0, 0);
    assert_eq!(type_of(&a, i_plus), "int");
    let tables = &a.unit().tables;
    let read = base(&a, *tables.read_element.get(i_plus).expect("read element"));
    let write = base(
        &a,
        *tables.write_element.get(i_plus).expect("write element"),
    );
    assert_eq!(read, write);
    assert_eq!(read.tag(), Tag::FormalParameter);
    assert_eq!(a.type_str(*tables.read_type.get(i_plus).unwrap()), "int");
    assert_eq!(a.type_str(*tables.write_type.get(i_plus).unwrap()), "int");
    let element = a.element_of(i_plus).expect("+ element");
    assert_eq!(element_desc(&a, element).0, "num.+");

    // `double - int` is `double`.
    assert_eq!(
        type_at(&a, NodeKind::AssignmentExpression, "d -= i"),
        "double"
    );

    // `a.x *= 2`: the getter and the setter of `x`.
    let x_times = a.node_at(NodeKind::AssignmentExpression, "a.x *= 2", 0, 0);
    assert_eq!(type_of(&a, x_times), "int");
    let read = base(&a, *tables.read_element.get(x_times).unwrap());
    let write = base(&a, *tables.write_element.get(x_times).unwrap());
    assert_eq!(element_desc(&a, read), ("A.x".to_string(), Tag::Getter));
    assert_eq!(element_desc(&a, write), ("A.x".to_string(), Tag::Setter));

    // `a.y ??= 1.5`: UP(NonNull(num?), double) = num.
    let y_if_null = a.node_at(NodeKind::AssignmentExpression, "a.y ??= 1.5", 0, 0);
    assert_eq!(type_of(&a, y_if_null), "num");
    assert_eq!(
        a.type_str(*tables.read_type.get(y_if_null).unwrap()),
        "num?"
    );
    assert_eq!(
        a.type_str(*tables.write_type.get(y_if_null).unwrap()),
        "num?"
    );

    // `n ??= i`: `int`, and `n` is promoted after it.
    assert_eq!(
        type_at(&a, NodeKind::AssignmentExpression, "n ??= i"),
        "int"
    );

    // A plain assignment has a write element but no read element.
    let assign = a.node_at(NodeKind::AssignmentExpression, "i = 3", 0, 0);
    assert_eq!(type_of(&a, assign), "int");
    assert!(tables.read_element.get(assign).is_none());
    assert!(tables.write_element.get(assign).is_some());
    assert!(
        a.diagnostic_names().is_empty(),
        "{:?}",
        a.diagnostic_names()
    );
}

#[test]
fn increments_resolve_the_operand_for_read_and_write() {
    let source = r#"
class A {
  int x = 0;
  num n = 0;
  void m() {
    x++;
    ++x;
    this.x--;
    n++;
  }
}
"#;
    let Some(a) = run(source) else {
        return;
    };
    let post = a.node_at(NodeKind::PostfixExpression, "x++", 0, 0);
    assert_eq!(type_of(&a, post), "int");
    let tables = &a.unit().tables;
    let read = base(&a, *tables.read_element.get(post).unwrap());
    let write = base(&a, *tables.write_element.get(post).unwrap());
    assert_eq!(element_desc(&a, read), ("A.x".to_string(), Tag::Getter));
    assert_eq!(element_desc(&a, write), ("A.x".to_string(), Tag::Setter));
    let element = a.element_of(post).expect("+ element");
    assert_eq!(element_desc(&a, element).0, "num.+");

    assert_eq!(type_at(&a, NodeKind::PrefixExpression, "++x"), "int");
    assert_eq!(type_at(&a, NodeKind::PostfixExpression, "this.x--"), "int");
    // `n++` has the type of the operand (`num`).
    assert_eq!(type_at(&a, NodeKind::PostfixExpression, "n++"), "num");
}

#[test]
fn property_accesses_with_null_shorting_super_and_statics() {
    let source = r#"
import 'dart:math' as math;

class A {
  int x = 0;
  A? next;
  static int s = 1;
  int get g => 0;
}

class B extends A {
  int get g => super.g + 1;
}

void f(A? a, A b, (int, {String name}) r) {
  a?.next?.x;
  a?.x;
  b.next!.x;
  A.s;
  math.pi;
  r.$1;
  r.name;
}
"#;
    let Some(a) = run(source) else {
        return;
    };
    // Null shorting: the whole chain is nullable, the inner parts are not.
    let chain = outermost_at(&a, NodeKind::PropertyAccess, "a?.next?.x");
    assert_eq!(type_of(&a, chain), "int?");
    // `a?.next` is `A?` (the declared type of `next`).
    assert_eq!(type_at(&a, NodeKind::PropertyAccess, "a?.next?.x"), "A?");
    assert_eq!(type_at(&a, NodeKind::PropertyAccess, "a?.x"), "int?");
    assert_eq!(type_at(&a, NodeKind::PropertyAccess, "b.next!.x"), "int");
    assert_eq!(type_at(&a, NodeKind::PostfixExpression, "b.next!"), "A");

    // `super.g`: the getter of `A`.
    let super_g = a.node_at(NodeKind::PropertyAccess, "super.g", 0, 0);
    assert_eq!(type_of(&a, super_g), "int");
    let name = a.node_at(NodeKind::SimpleIdentifier, "g + 1", 0, 0);
    let element = a.element_of(name).expect("super getter");
    assert_eq!(element_desc(&a, element), ("A.g".to_string(), Tag::Getter));

    // Static access on a class and a prefixed top-level getter.
    assert_eq!(type_at(&a, NodeKind::PrefixedIdentifier, "A.s"), "int");
    let pi = a.node_at(NodeKind::PrefixedIdentifier, "math.pi", 0, 0);
    assert_eq!(type_of(&a, pi), "double");
    let name = a.node_at(NodeKind::SimpleIdentifier, "pi;", 0, 0);
    let element = a.element_of(name).expect("pi");
    assert_eq!(a.element_name(element).as_deref(), Some("pi"));

    // Record fields: the prefixed identifiers are rewritten to property
    // accesses.
    assert_eq!(type_at(&a, NodeKind::PropertyAccess, "r.$1"), "int");
    assert_eq!(type_at(&a, NodeKind::PropertyAccess, "r.name"), "String");
    assert!(
        a.diagnostic_names().is_empty(),
        "{:?}",
        a.diagnostic_names()
    );
}

#[test]
fn index_expressions_use_the_substituted_operators() {
    let source = r#"
void f(List<int> l, Map<String, double> m, List<int>? n) {
  l[0];
  m['a'] = 1.0;
  l[0] += 1;
  n?[0];
}
"#;
    let Some(a) = run(source) else {
        return;
    };
    let read = a.node_at(NodeKind::IndexExpression, "l[0];", 0, 0);
    assert_eq!(type_of(&a, read), "int");
    // The element is `List.[]` substituted with `E: int`.
    let element = *a.unit().tables.element.get(read).expect("[] element");
    assert!(matches!(element, ElemRef::Member(_)));
    assert_eq!(element_desc(&a, base(&a, element)).0, "List.[]");

    let assign = a.node_at(NodeKind::AssignmentExpression, "m['a'] = 1.0", 0, 0);
    assert_eq!(type_of(&a, assign), "double");
    let tables = &a.unit().tables;
    let write = base(&a, *tables.write_element.get(assign).unwrap());
    assert_eq!(element_desc(&a, write).0, "Map.[]=");
    assert_eq!(
        a.type_str(*tables.write_type.get(assign).unwrap()),
        "double"
    );

    let compound = a.node_at(NodeKind::AssignmentExpression, "l[0] += 1", 0, 0);
    assert_eq!(type_of(&a, compound), "int");
    let read = base(&a, *tables.read_element.get(compound).unwrap());
    assert_eq!(element_desc(&a, read).0, "List.[]");

    assert_eq!(type_at(&a, NodeKind::IndexExpression, "n?[0]"), "int?");
}

#[test]
fn errors_of_property_and_operator_resolution_are_reported() {
    let source = r#"
class A {
  final int f = 0;
}

void g(A a, int? n, String s) {
  a.y;
  n.isEven;
  a.f = 1;
  s - 1;
  final x = 0;
  x = 1;
}
"#;
    let Some(a) = run(source) else {
        return;
    };
    let offset = |search: &str, delta: usize| a.source().find(search).unwrap() + delta;
    let names = a.diagnostic_names();
    let expected = [
        format!("undefined_getter@{}", offset("a.y", 2)),
        format!("unchecked_use_of_nullable_value@{}", offset("n.isEven", 2)),
        format!("assignment_to_final@{}", offset("a.f = 1", 2)),
        format!("undefined_operator@{}", offset("s - 1", 2)),
        format!("assignment_to_final_local@{}", offset("x = 1", 0)),
    ];
    for e in &expected {
        assert!(names.contains(e), "missing {e} in {names:?}");
    }
    // The unresolved getter has no element and the invalid type.
    assert_eq!(
        type_at(&a, NodeKind::PrefixedIdentifier, "a.y"),
        "InvalidType"
    );
}

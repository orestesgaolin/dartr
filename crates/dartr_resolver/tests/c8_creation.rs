//! Integration tests of unit C8: instance creation (with the inference of
//! the type arguments of the constructed type), constructor references,
//! function references and the type literal rewrite, implicit call
//! references, dot shorthands, `super(...)` / `this(...)` constructor
//! invocations and enum constant arguments. Each test analyzes a small
//! library against the SDK and checks static types, elements and rewritten
//! node kinds. The expected values are those of the Dart analyzer (the
//! `assertResolvedNodeText` dumps of
//! `pkg/analyzer/test/src/dart/resolution/*_test.dart`).
//!
//! The differential fixtures in `tests/fixtures/c8` (the code of 592
//! analyzer resolution tests) are compared with the analyzer by
//! `difftest resolved` / `difftest resolved-el`.

mod support;

use dartr_ast::NodeKind;
use dartr_element::{ElemRef, Tag};
use support::{Analyzed, analyze};

fn run(source: &str) -> Option<Analyzed> {
    let a = analyze(&[("main.dart", source)]);
    if a.is_none() {
        eprintln!("skipped: no Dart SDK on PATH");
    }
    a
}

/// The static type of the node of [kind] at the [n]th occurrence of
/// [search], as a display string.
fn type_at(a: &Analyzed, kind: NodeKind, search: &str, n: usize) -> String {
    let node = a.node_at(kind, search, n, 0);
    let t = *a
        .unit()
        .tables
        .static_type
        .get(node)
        .unwrap_or_else(|| panic!("no static type for {kind:?} at {search:?}"));
    a.type_str(t)
}

/// The element of the node of [kind] at [search] (`ResolutionTables`
/// element, with its substitution), and the display string of its type
/// (`member::type_`).
fn element_type_at(a: &Analyzed, kind: NodeKind, search: &str) -> (ElemRef, String) {
    let node = a.node_at(kind, search, 0, 0);
    let e = *a
        .unit()
        .tables
        .element
        .get(node)
        .unwrap_or_else(|| panic!("no element for {kind:?} at {search:?}"));
    let unit = a.unit();
    let t = dartr_typesystem::member::type_(&a.ctx(unit), e);
    (e, a.type_str(t))
}

#[test]
fn instance_creation_infers_type_arguments_from_arguments_and_context() {
    // instance_creation_test.dart: test_generic_inferTypeArguments,
    // test_namedArgument; context: `List<num> = List.filled(...)`.
    let source = r#"
class A<T> {
  A(T a);
  A.named({required T value});
}
void f() {
  A(0);
  A.named(value: 'x');
  new A<double>(1.5);
  List<num> l = List.filled(2, 0);
  const c = A(true);
}
"#;
    let Some(a) = run(source) else { return };
    assert_eq!(
        type_at(&a, NodeKind::InstanceCreationExpression, "A(0)", 0),
        "A<int>"
    );
    assert_eq!(
        type_at(&a, NodeKind::InstanceCreationExpression, "A.named(value", 0),
        "A<String>"
    );
    assert_eq!(
        type_at(&a, NodeKind::InstanceCreationExpression, "new A<double>", 0),
        "A<double>"
    );
    assert_eq!(
        type_at(&a, NodeKind::InstanceCreationExpression, "List.filled", 0),
        "List<num>"
    );
    assert_eq!(
        type_at(&a, NodeKind::InstanceCreationExpression, "A(true)", 0),
        "A<bool>"
    );

    // The constructor name element is the constructor substituted with the
    // inferred type arguments.
    let (e, t) = element_type_at(&a, NodeKind::ConstructorName, "A.named(value");
    assert!(
        matches!(e, ElemRef::Member(_)),
        "a substituted constructor: {e:?}"
    );
    assert_eq!(t, "A<String> Function({required String value})");
    let (_, t) = element_type_at(&a, NodeKind::ConstructorName, "List.filled");
    assert_eq!(t, "List<num> Function(int, num, {bool growable})");
    // The argument got the parameter type as context: `0` is an `int`
    // passed to `num`.
    assert_eq!(
        type_at(&a, NodeKind::IntegerLiteral, "0);\n  const", 0),
        "int"
    );
}

#[test]
fn instance_creation_of_type_alias_and_prefixed_class() {
    // instance_creation_test.dart: test_typeAlias_generic_class_generic_*,
    // test_importPrefix.
    let source = r#"
import 'dart:core' as core;
class A<T, U> {
  A(T t, U u);
}
typedef B<V> = A<V, core.String>;
void f() {
  B(0, '');
  core.List<core.int>.empty();
}
"#;
    let Some(a) = run(source) else { return };
    assert_eq!(
        type_at(&a, NodeKind::InstanceCreationExpression, "B(0", 0),
        "A<int, String>"
    );
    assert_eq!(
        type_at(
            &a,
            NodeKind::InstanceCreationExpression,
            "core.List<core.int>.empty",
            0
        ),
        "List<int>"
    );
}

#[test]
fn constructor_reference_infers_from_context() {
    // constructor_reference_test.dart: test_class_generic_inferFromContext,
    // test_class_generic_named, test_class_nonGeneric_unnamed.
    let source = r#"
class A<T> {
  A.foo(T a);
}
class B {
  B();
}
void f() {
  A<int> Function(int) x = A.foo;
  var y = A<String>.foo;
  var z = B.new;
}
"#;
    let Some(a) = run(source) else { return };
    assert_eq!(
        type_at(&a, NodeKind::ConstructorReference, "A.foo;", 0),
        "A<int> Function(int)"
    );
    assert_eq!(
        type_at(&a, NodeKind::ConstructorReference, "A<String>.foo", 0),
        "A<String> Function(String)"
    );
    assert_eq!(
        type_at(&a, NodeKind::ConstructorReference, "B.new", 0),
        "B Function()"
    );
}

#[test]
fn function_reference_instantiates_and_type_literals_are_rewritten() {
    // function_reference_test.dart: test_localFunction,
    // test_staticMethod; type_literal_test.dart: test_class,
    // test_typeAlias_typeVariable.
    let source = r#"
T id<T>(T a) => a;
class C {
  static S s<S>(S a) => a;
  void m() {
    var a = s<int>;
  }
}
typedef Fn<T> = T Function(T);
void f() {
  var b = id<String>;
  var c = Map<int, bool>;
  var d = List<int>;
  var e = Fn<int>;
}
"#;
    let Some(a) = run(source) else { return };
    assert_eq!(
        type_at(&a, NodeKind::FunctionReference, "id<String>", 0),
        "String Function(String)"
    );
    assert_eq!(
        type_at(&a, NodeKind::FunctionReference, "s<int>", 0),
        "int Function(int)"
    );
    // `Map<int, bool>` and `List<int>` in an expression are type literals,
    // not function references.
    for search in ["Map<int, bool>", "List<int>;", "Fn<int>;"] {
        assert_eq!(
            type_at(&a, NodeKind::TypeLiteral, search, 0),
            "Type",
            "{search}"
        );
        let literal = a.node_at(NodeKind::TypeLiteral, search, 0, 0);
        let named_type = a.unit().ast.children(literal)[0];
        assert_eq!(a.unit().ast.kind(named_type), NodeKind::NamedType);
        assert!(
            a.element_of(named_type).is_some(),
            "{search}: the named type has an element"
        );
    }
    let literal = a.node_at(NodeKind::TypeLiteral, "Map<int, bool>", 0, 0);
    let named_type = a.unit().ast.children(literal)[0];
    assert_eq!(
        a.annotation_type_str(named_type).as_deref(),
        Some("Map<int, bool>")
    );
    let literal = a.node_at(NodeKind::TypeLiteral, "Fn<int>", 0, 0);
    let named_type = a.unit().ast.children(literal)[0];
    assert_eq!(
        a.annotation_type_str(named_type).as_deref(),
        Some("int Function(int)")
    );
}

#[test]
fn implicit_call_references_are_inserted_for_function_contexts() {
    // function_reference_test.dart: test_implicitCallTearoff,
    // test_implicitCallTearoff_tooFewTypeArguments-like instantiation.
    let source = r#"
class C {
  T call<T>(T t) => t;
}
void f(C c) {
  int Function(int) a = c;
  var b = c<String>;
}
"#;
    let Some(a) = run(source) else { return };
    assert_eq!(
        type_at(&a, NodeKind::ImplicitCallReference, "c;", 0),
        "int Function(int)"
    );
    assert_eq!(
        type_at(&a, NodeKind::ImplicitCallReference, "c<String>", 0),
        "String Function(String)"
    );
    let node = a.node_at(NodeKind::ImplicitCallReference, "c;", 0, 0);
    let call = a.element_of(node).expect("the call method");
    assert_eq!(call.tag(), Tag::Method);
    assert_eq!(a.element_name(call).as_deref(), Some("call"));
}

#[test]
fn dot_shorthands_resolve_in_the_context_type() {
    // dot_shorthand_property_access_test.dart: test_basic;
    // dot_shorthand_invocation_test.dart: test_basic,
    // test_constructor_rewrite; dot_shorthand_constructor_invocation_test:
    // test_new, test_const.
    let source = r#"
class C<T> {
  final T v;
  const C(this.v);
  C.named(this.v);
  static C<int> get zero => C(0);
  static C<int> make(int x) => C(x);
}
void f() {
  C<int> a = .zero;
  C<int> b = .make(1);
  C<String> c = .named('x');
  C<num> d = .new(2);
  C<int> e = const .new(3);
  C<int> Function(int) g = .named;
}
"#;
    let Some(a) = run(source) else { return };
    assert_eq!(
        type_at(&a, NodeKind::DotShorthandPropertyAccess, ".zero", 0),
        "C<int>"
    );
    assert_eq!(
        type_at(&a, NodeKind::DotShorthandInvocation, ".make", 0),
        "C<int>"
    );
    let invocation = a.node_at(NodeKind::DotShorthandInvocation, ".make", 0, 0);
    let invoke_type = a
        .unit()
        .tables
        .invoke_type
        .get(invocation)
        .copied()
        .unwrap();
    assert_eq!(a.type_str(invoke_type), "C<int> Function(int)");
    // `.named('x')` is parsed as a dot shorthand invocation and rewritten to
    // a constructor invocation.
    assert_eq!(
        type_at(
            &a,
            NodeKind::DotShorthandConstructorInvocation,
            ".named('x')",
            0
        ),
        "C<String>"
    );
    assert_eq!(
        type_at(
            &a,
            NodeKind::DotShorthandConstructorInvocation,
            ".new(2)",
            0
        ),
        "C<num>"
    );
    assert_eq!(
        type_at(
            &a,
            NodeKind::DotShorthandConstructorInvocation,
            "const .new(3)",
            0
        ),
        "C<int>"
    );
    // A function type is not a context of a dot shorthand (the analyzer
    // gives `InvalidType` and reports the missing context).
    assert_eq!(
        type_at(&a, NodeKind::DotShorthandPropertyAccess, ".named;", 0),
        "InvalidType"
    );
    // The constructor name of the rewritten invocation is the substituted
    // constructor.
    let (_, t) = element_type_at(&a, NodeKind::SimpleIdentifier, "named('x')");
    assert_eq!(t, "C<String> Function(String)");
    // The unused locals are reported by the wave D warnings, like the analyzer.
    let names: Vec<String> = a
        .diagnostic_names()
        .into_iter()
        .filter(|n| !n.starts_with("unused_local_variable@"))
        .collect();
    assert_eq!(names.len(), 1, "{names:?}");
    assert!(
        names[0].starts_with("dot_shorthand_missing_context@"),
        "{names:?}"
    );
}

#[test]
fn super_and_redirecting_constructor_invocations_have_elements_and_contexts() {
    // super_constructor_invocation_test.dart: test_named, test_unnamed;
    // redirecting_constructor_invocation_test.dart: test_named.
    let source = r#"
class A<T> {
  A(List<T> a);
  A.named(T a);
}
class B extends A<num> {
  B() : super([1]);
  B.n() : super.named(2);
  B.r() : this.n();
}
"#;
    let Some(a) = run(source) else { return };
    // `super([1])`: the list literal gets the parameter type `List<num>` as
    // context (A<num> substituted).
    let (e, t) = element_type_at(&a, NodeKind::SuperConstructorInvocation, "super([1])");
    assert!(matches!(e, ElemRef::Member(_)));
    assert_eq!(t, "A<num> Function(List<num>)");
    let (_, t) = element_type_at(&a, NodeKind::SimpleIdentifier, "named(2)");
    assert_eq!(t, "A<num> Function(num)");
    let (e, t) = element_type_at(&a, NodeKind::RedirectingConstructorInvocation, "this.n()");
    assert!(matches!(e, ElemRef::Base(_)));
    assert_eq!(t, "B Function()");
    assert_eq!(type_at(&a, NodeKind::IntegerLiteral, "2)", 0), "int");
}

#[test]
fn super_constructor_invocation_reports_undefined_constructor() {
    // super_constructor_invocation_test.dart (diagnostics of
    // ElementResolver.visitSuperConstructorInvocation).
    let source = r#"
class A {
  A();
}
class B extends A {
  B() : super.missing();
}
"#;
    let Some(a) = run(source) else { return };
    let names = a.diagnostic_names();
    assert!(
        names
            .iter()
            .any(|n| n.starts_with("undefined_constructor_in_initializer@")),
        "{names:?}"
    );
}

#[test]
fn enum_constant_arguments_get_parameter_contexts() {
    // enum_test.dart: test_constructor_argumentList_contextType,
    // test_constructor_generic_noTypeArguments_named.
    let source = r#"
enum E<T> {
  a(1.0),
  b.named([2]);
  const E(double x);
  const E.named(List<T> x);
}
"#;
    let Some(a) = run(source) else { return };
    // The context `double` makes `1.0` a double, and the list literal is
    // resolved with the parameter type of the constructor.
    assert_eq!(type_at(&a, NodeKind::DoubleLiteral, "1.0", 0), "double");
    let constant = a.node_at(NodeKind::EnumConstantDeclaration, "b.named", 0, 0);
    let constructor = a.element_of(constant).expect("constructor element");
    assert_eq!(constructor.tag(), Tag::Constructor);
    assert_eq!(a.element_name(constructor).as_deref(), Some("named"));
}

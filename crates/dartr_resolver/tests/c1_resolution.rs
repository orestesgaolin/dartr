//! Integration tests of unit C1: element binding, scopes, type annotations,
//! AST rewrites. Each test links a small library against the SDK, runs the
//! library analyzer and checks concrete results of the first two passes
//! (`declared_fragment`, element types, `annotation_type`,
//! `scope_lookup_result`, `element`, diagnostics).

mod support;

use dartr_ast::NodeKind;
use dartr_element::{AnyElement, Tag};
use support::{Analyzed, analyze};

fn run(files: &[(&str, &str)]) -> Option<Analyzed> {
    let a = analyze(files);
    if a.is_none() {
        eprintln!("skipped: no Dart SDK on PATH");
    }
    a
}

/// The type of a variable element, as a display string.
fn variable_type_str(a: &Analyzed, e: dartr_element::ElementId) -> String {
    let unit = a.unit();
    let ctx = a.ctx(unit);
    let t = dartr_resolver::element_ext::variable_type(&ctx, e);
    a.type_str(t)
}

#[test]
fn local_variables_are_bound_with_types_and_scope_lookups() {
    let source = r#"
int f(int a) {
  var x = a;
  final List<String> y = [];
  return x;
}
"#;
    let Some(a) = run(&[("main.dart", source)]) else {
        return;
    };

    // `var x`: a local fragment and element, implicit type, `dynamic`
    // until inference.
    let x_decl = a.node_at(NodeKind::VariableDeclaration, "x = a", 0, 0);
    let x = a.declared_element(x_decl).expect("x is bound");
    assert_eq!(x.tag(), Tag::LocalVariable);
    assert_eq!(a.element_name(x).as_deref(), Some("x"));
    assert_eq!(variable_type_str(&a, x), "dynamic");

    // `final List<String> y`: the explicit type.
    let y_decl = a.node_at(NodeKind::VariableDeclaration, "y = []", 0, 0);
    let y = a.declared_element(y_decl).expect("y is bound");
    assert_eq!(variable_type_str(&a, y), "List<String>");
    {
        let unit = a.unit();
        let ctx = a.ctx(unit);
        let first = ctx.element_data(y).unwrap().first_fragment;
        assert!(dartr_resolver::element_ext::is_final(&ctx, y));
        assert!(
            !ctx.fragment_data(first)
                .unwrap()
                .flags
                .has(dartr_element::FragmentFlags::VARIABLE_FRAGMENT_HAS_IMPLICIT_TYPE)
        );
    }

    // `a` in the initializer of `x`: the linked parameter of `f`.
    let a_ref = a.node_at(NodeKind::SimpleIdentifier, "a;", 0, 0);
    let lookup = a
        .unit()
        .rt
        .scope_lookup_result
        .get(a_ref)
        .copied()
        .expect("looked up");
    let a_param = lookup.getter.expect("a found");
    assert_eq!(a_param.tag(), Tag::FormalParameter);
    assert_eq!(a.element_name(a_param).as_deref(), Some("a"));
    assert_eq!(
        a.element_of(a_ref),
        Some(a_param),
        "promotable: element set"
    );

    // `x` in `return x`: the local element.
    let x_ref = a.node_at(NodeKind::SimpleIdentifier, "x;", 0, 0);
    assert_eq!(
        a.unit().rt.scope_lookup_result.get(x_ref).unwrap().getter,
        Some(x)
    );

    // `List<String>`: the named type has the class element and the type.
    let list_type = a.node_at(NodeKind::NamedType, "List<String>", 0, 0);
    assert_eq!(
        a.annotation_type_str(list_type).as_deref(),
        Some("List<String>")
    );
    let list_element = a.element_of(list_type).expect("List element");
    assert_eq!(a.element_name(list_element).as_deref(), Some("List"));
}

#[test]
fn local_function_has_its_type_parameters_and_formal_parameters() {
    let source = r#"
void f() {
  T g<T>(T t, [int n = 0]) => t;
  g(1);
}
"#;
    let Some(a) = run(&[("main.dart", source)]) else {
        return;
    };
    let g_decl = a.node_at(NodeKind::FunctionDeclaration, "T g<T>", 0, 0);
    let g = a.declared_element(g_decl).expect("g is bound");
    assert_eq!(g.tag(), Tag::LocalFunction);
    let unit = a.unit();
    let ctx = a.ctx(unit);
    let AnyElement::LocalFunction(data) = ctx.any(g) else {
        panic!("local function");
    };
    assert_eq!(data.type_params.len(), 1);
    assert_eq!(
        a.element_name(data.type_params[0].raw()).as_deref(),
        Some("T")
    );
    let params: Vec<String> = data
        .formal_params
        .iter()
        .map(|p| a.element_name(p.raw()).unwrap())
        .collect();
    assert_eq!(params, ["t", "n"]);
    // The parameter types are set by the resolution visitor.
    assert_eq!(variable_type_str(&a, data.formal_params[0].raw()), "T");
    assert_eq!(variable_type_str(&a, data.formal_params[1].raw()), "int");
    assert_eq!(a.type_str(data.return_type.get().unwrap()), "T");
    // The function expression shares the fragment of the declaration.
    let expression = a.node_at(NodeKind::FunctionExpression, "<T>(T t", 0, 0);
    assert_eq!(a.declared_element(expression), Some(g));
    // `g` in `g(1)` finds the local function.
    let g_ref = a.node_at(NodeKind::SimpleIdentifier, "g(1)", 0, 0);
    assert_eq!(
        unit.rt.scope_lookup_result.get(g_ref).unwrap().getter,
        Some(g)
    );
}

#[test]
fn type_annotations_of_function_and_record_types() {
    let source = r#"
int Function(String s)? f1;
(int, {String name})? r1;
void g(void Function<T>(T) callback, int h(String s)) {}
"#;
    let Some(a) = run(&[("main.dart", source)]) else {
        return;
    };
    let function_type = a.node_at(
        NodeKind::GenericFunctionType,
        "int Function(String s)?",
        0,
        0,
    );
    assert_eq!(
        a.annotation_type_str(function_type).as_deref(),
        Some("int Function(String)?")
    );
    let record = a.node_at(
        NodeKind::RecordTypeAnnotation,
        "(int, {String name})?",
        0,
        0,
    );
    assert_eq!(
        a.annotation_type_str(record).as_deref(),
        Some("(int, {String name})?")
    );
    let generic = a.node_at(NodeKind::GenericFunctionType, "void Function<T>(T)", 0, 0);
    assert_eq!(
        a.annotation_type_str(generic).as_deref(),
        Some("void Function<T>(T)")
    );
    // A function-typed formal parameter: the explicit fragment type is on
    // the formal parameter node.
    let h = a.node_at(NodeKind::RegularFormalParameter, "int h(String s)", 0, 0);
    assert_eq!(
        a.annotation_type_str(h).as_deref(),
        Some("int Function(String)")
    );
}

#[test]
fn import_prefix_and_prefixed_named_type() {
    let source = r#"
import 'dart:async' as async;
async.Future<int>? v;
"#;
    let Some(a) = run(&[("main.dart", source)]) else {
        return;
    };
    let named_type = a.node_at(NodeKind::NamedType, "async.Future<int>?", 0, 0);
    assert_eq!(
        a.annotation_type_str(named_type).as_deref(),
        Some("Future<int>?")
    );
    let future = a.element_of(named_type).expect("Future element");
    assert_eq!(a.element_name(future).as_deref(), Some("Future"));
    let import_prefix = a.node_at(NodeKind::ImportPrefixReference, "async.Future", 0, 0);
    let prefix = a.element_of(import_prefix).expect("prefix element");
    assert_eq!(prefix.tag(), Tag::Prefix);
    assert_eq!(a.element_name(prefix).as_deref(), Some("async"));
    // The prefix identifier of the import directive gets the same element.
    let directive_prefix = a.node_at(NodeKind::SimpleIdentifier, "async;", 0, 0);
    assert_eq!(a.element_of(directive_prefix), Some(prefix));
}

#[test]
fn conflicting_imports_give_one_multiply_defined_element_per_unit() {
    let main = r#"
import 'a.dart';
import 'b.dart';
C? c1;
void f() {
  C;
  C;
}
"#;
    let Some(a) = run(&[
        ("main.dart", main),
        ("a.dart", "class C {}\n"),
        ("b.dart", "class C {}\n"),
    ]) else {
        return;
    };
    let named_type = a.node_at(NodeKind::NamedType, "C? c1", 0, 0);
    let element = a.element_of(named_type).expect("an element");
    assert_eq!(element.tag(), Tag::MultiplyDefined);
    assert_eq!(
        a.annotation_type_str(named_type).as_deref(),
        Some("InvalidType")
    );
    {
        let unit = a.unit();
        let ctx = a.ctx(unit);
        let AnyElement::MultiplyDefined(m) = ctx.any(element) else {
            panic!("multiply defined");
        };
        let conflicting: Vec<String> = m
            .conflicting_elements
            .iter()
            .map(|&e| {
                let library = ctx.element_data(e).unwrap().library.unwrap();
                let uri = &ctx.fragment(ctx.get(library).first_fragment()).source.uri;
                uri.rsplit('/').next().unwrap().to_string()
            })
            .collect();
        assert_eq!(conflicting, ["a.dart", "b.dart"]);
    }
    // Each lookup in the unit gives the same element (Dart identity).
    let first = a.node_at(NodeKind::SimpleIdentifier, "C;", 0, 0);
    let second = a.node_at(NodeKind::SimpleIdentifier, "C;", 1, 0);
    let rt = &a.unit().rt;
    assert_eq!(
        rt.scope_lookup_result.get(first).unwrap().getter,
        Some(element)
    );
    assert_eq!(
        rt.scope_lookup_result.get(second).unwrap().getter,
        Some(element)
    );
}

#[test]
fn instance_members_hide_outer_names_and_static_members_are_found() {
    let source = r#"
int x = 0;
class A {
  int x = 1;
  static int s = 2;
  void m() {
    x;
    s;
  }
}
"#;
    let Some(a) = run(&[("main.dart", source)]) else {
        return;
    };
    let rt = &a.unit().rt;
    // Dart `InstanceScope.lookup`: an instance member gives an empty result.
    let x_ref = a.node_at(NodeKind::SimpleIdentifier, "x;", 0, 0);
    let x_lookup = rt.scope_lookup_result.get(x_ref).copied().unwrap();
    assert_eq!(x_lookup.getter, None);
    assert_eq!(x_lookup.setter, None);
    // A static member is found.
    let s_ref = a.node_at(NodeKind::SimpleIdentifier, "s;", 0, 0);
    let s = rt
        .scope_lookup_result
        .get(s_ref)
        .unwrap()
        .getter
        .expect("s");
    assert_eq!(s.tag(), Tag::Getter);
    assert_eq!(a.element_name(s).as_deref(), Some("s"));
}

#[test]
fn method_invocation_of_a_named_constructor_is_rewritten() {
    let source = r#"
class A {
  A.named();
}
var a = A.named();
"#;
    let Some(a) = run(&[("main.dart", source)]) else {
        return;
    };
    // The parser made a method invocation; the rewrite made it an instance
    // creation with the class as the type.
    let creation = a.node_at(NodeKind::InstanceCreationExpression, "A.named();\n", 1, 0);
    let ast = &a.unit().ast;
    let constructor_name = ast
        [dartr_ast::Id::<dartr_ast::InstanceCreationExpression>::from_raw(creation)]
    .constructor_name;
    let named_type = ast[constructor_name].type_;
    assert_eq!(
        a.annotation_type_str(named_type.raw()).as_deref(),
        Some("A")
    );
    assert!(
        support::find_node(
            ast,
            a.unit().unit.raw(),
            NodeKind::MethodInvocation,
            creation_offset(&a, "A.named();\n")
        )
        .is_none(),
        "the method invocation is not in the tree"
    );
}

fn creation_offset(a: &Analyzed, search: &str) -> u32 {
    a.source().rfind(search).unwrap() as u32
}

#[test]
fn break_and_continue_targets_and_labels() {
    let source = r#"
void f() {
  outer: while (true) {
    for (;;) {
      break outer;
    }
    continue undefined;
  }
}
"#;
    let Some(a) = run(&[("main.dart", source)]) else {
        return;
    };
    let break_statement = a.node_at(NodeKind::BreakStatement, "break outer", 0, 0);
    let while_statement = a.node_at(NodeKind::WhileStatement, "while (true)", 0, 0);
    assert_eq!(
        a.unit()
            .rt
            .break_continue_target
            .get(break_statement)
            .copied(),
        Some(while_statement)
    );
    let label_reference = a.node_at(NodeKind::LabelReference, "outer;", 0, 0);
    let label = a.element_of(label_reference).expect("label element");
    assert_eq!(label.tag(), Tag::Label);
    assert_eq!(a.element_name(label).as_deref(), Some("outer"));
    let offset = a.source().find("undefined").unwrap();
    assert!(
        a.diagnostic_names()
            .contains(&format!("label_undefined@{offset}")),
        "{:?}",
        a.diagnostic_names()
    );
}

#[test]
fn pattern_variables_are_declared_in_guarded_scope() {
    let source = r#"
void f(Object o) {
  if (o case int i when i > 0) {
    i;
  }
  var (p, q) = (1, 2);
  p;
}
"#;
    let Some(a) = run(&[("main.dart", source)]) else {
        return;
    };
    let pattern = a.node_at(NodeKind::DeclaredVariablePattern, "int i when", 0, 0);
    let i = a.declared_element(pattern).expect("i is bound");
    assert_eq!(i.tag(), Tag::BindPatternVariable);
    assert_eq!(variable_type_str(&a, i), "int");
    let in_guard = a.node_at(NodeKind::SimpleIdentifier, "i > 0", 0, 0);
    let in_body = a.node_at(NodeKind::SimpleIdentifier, "i;", 0, 0);
    let rt = &a.unit().rt;
    assert_eq!(
        rt.scope_lookup_result.get(in_guard).unwrap().getter,
        Some(i)
    );
    assert_eq!(rt.scope_lookup_result.get(in_body).unwrap().getter, Some(i));

    let p_pattern = a.node_at(NodeKind::DeclaredVariablePattern, "p, q", 0, 0);
    let p = a.declared_element(p_pattern).expect("p");
    let p_ref = a.node_at(NodeKind::SimpleIdentifier, "p;", 0, 0);
    assert_eq!(rt.scope_lookup_result.get(p_ref).unwrap().getter, Some(p));
    let declaration = a.node_at(NodeKind::PatternVariableDeclaration, "var (p, q)", 0, 0);
    let elements = rt
        .pattern_variable_declaration_elements
        .get(declaration)
        .unwrap();
    let names: Vec<String> = elements
        .iter()
        .map(|&e| a.element_name(e).unwrap())
        .collect();
    assert_eq!(names, ["p", "q"]);
}

#[test]
fn undefined_types_and_non_types_are_reported() {
    let source = r#"
Undefined u;
int v = 0;
v w;
"#;
    let Some(a) = run(&[("main.dart", source)]) else {
        return;
    };
    let undefined = a.source().find("Undefined").unwrap();
    let not_a_type = a.source().find("v w").unwrap();
    let names = a.diagnostic_names();
    assert!(
        names.contains(&format!("undefined_class@{undefined}")),
        "{names:?}"
    );
    assert!(
        names.contains(&format!("not_a_type@{not_a_type}")),
        "{names:?}"
    );
    let named_type = a.node_at(NodeKind::NamedType, "Undefined", 0, 0);
    assert_eq!(
        a.annotation_type_str(named_type).as_deref(),
        Some("InvalidType")
    );
}

#[test]
fn local_declarations_in_blocks_are_visible_before_their_declaration() {
    // Dart defines all local functions and variables of a block in its
    // scope before it visits the statements (forward references are
    // reported later by the resolver).
    let source = r#"
void f() {
  g;
  void g() {}
  {
    var g = 1;
    g;
  }
}
"#;
    let Some(a) = run(&[("main.dart", source)]) else {
        return;
    };
    let rt = &a.unit().rt;
    let outer_ref = a.node_at(NodeKind::SimpleIdentifier, "g;", 0, 0);
    let inner_ref = a.node_at(NodeKind::SimpleIdentifier, "g;", 1, 0);
    let function = a.declared_element(a.node_at(NodeKind::FunctionDeclaration, "void g()", 0, 0));
    let variable = a.declared_element(a.node_at(NodeKind::VariableDeclaration, "g = 1", 0, 0));
    assert_eq!(
        rt.scope_lookup_result.get(outer_ref).unwrap().getter,
        function
    );
    assert_eq!(
        rt.scope_lookup_result.get(inner_ref).unwrap().getter,
        variable
    );
    assert_ne!(function, variable);
}

#[test]
fn class_members_are_bound_to_linked_fragments() {
    let source = r#"
class A<T> {
  final T value;
  A(this.value);
  T get v => value;
  set v(T x) {}
  void m<U>(U u) {}
}
"#;
    let Some(a) = run(&[("main.dart", source)]) else {
        return;
    };
    let class = a
        .declared_element(a.node_at(NodeKind::ClassDeclaration, "class A", 0, 0))
        .unwrap();
    assert_eq!(class.tag(), Tag::Class);
    assert_eq!(a.element_str(class), "class A<T>");
    let getter = a
        .declared_element(a.node_at(NodeKind::MethodDeclaration, "T get v", 0, 0))
        .unwrap();
    assert_eq!(getter.tag(), Tag::Getter);
    let setter = a
        .declared_element(a.node_at(NodeKind::MethodDeclaration, "set v", 0, 0))
        .unwrap();
    assert_eq!(setter.tag(), Tag::Setter);
    let method = a
        .declared_element(a.node_at(NodeKind::MethodDeclaration, "void m", 0, 0))
        .unwrap();
    assert_eq!(method.tag(), Tag::Method);
    let field_formal = a
        .declared_element(a.node_at(NodeKind::FieldFormalParameter, "this.value", 0, 0))
        .unwrap();
    assert_eq!(field_formal.tag(), Tag::FieldFormalParameter);
    // Linked elements are not local.
    assert!(!field_formal.store().is_local());
    // `U` in the parameter type finds the type parameter of `m`.
    let u_type = a.node_at(NodeKind::NamedType, "U u", 0, 0);
    let u = a.element_of(u_type).unwrap();
    assert_eq!(u.tag(), Tag::TypeParameter);
    assert_eq!(a.annotation_type_str(u_type).as_deref(), Some("U"));
}

/// The diagnostics of the C1 passes on `c1_diagnostics.dart` are the ones
/// that `dart analyze --format=machine` (3.13.3) reports for these codes:
/// code, line and column.
#[test]
fn diagnostics_match_dart_analyze() {
    let source = include_str!("c1_diagnostics.dart");
    let Some(a) = run(&[("main.dart", source)]) else {
        return;
    };
    // Codes of other passes (unused elements, dead code, assignments).
    let other_passes = [
        "UNUSED_ELEMENT",
        "UNUSED_LOCAL_VARIABLE",
        "DEAD_CODE",
        "INVALID_ASSIGNMENT",
    ];
    let line_starts: Vec<usize> = std::iter::once(0)
        .chain(source.match_indices('\n').map(|(i, _)| i + 1))
        .collect();
    let mut actual: Vec<String> = a
        .unit()
        .diagnostics
        .iter()
        .map(|d| {
            let line = line_starts.iter().rposition(|&s| s <= d.offset).unwrap();
            let column = d.offset - line_starts[line] + 1;
            format!("{} {}:{}", d.code.name.to_uppercase(), line + 1, column)
        })
        .filter(|s| !other_passes.iter().any(|c| s.starts_with(c)))
        .collect();
    actual.sort();
    let mut expected = vec![
        "EXTENDS_NON_CLASS 5:17",
        "IMPLEMENTS_NON_CLASS 6:20",
        "MIXIN_OF_NON_CLASS 7:14",
        "NULLABLE_TYPE_IN_EXTENDS_CLAUSE 8:17",
        "UNDEFINED_CLASS 9:1",
        "NOT_A_TYPE 10:1",
        "WRONG_NUMBER_OF_TYPE_ARGUMENTS 11:1",
        "UNDEFINED_CLASS 12:1",
        "LABEL_IN_OUTER_SCOPE 17:13",
        "CONTINUE_LABEL_INVALID 22:7",
        "LABEL_UNDEFINED 25:9",
        "DUPLICATE_VARIABLE_PATTERN 26:26",
        "MISSING_VARIABLE_PATTERN 27:14",
        "MISSING_VARIABLE_PATTERN 27:23",
        "PATTERN_VARIABLE_ASSIGNMENT_INSIDE_GUARD 28:26",
        "TYPE_TEST_WITH_UNDEFINED_NAME 29:8",
        "CAST_TO_NON_TYPE 30:8",
        "NON_TYPE_AS_TYPE_ARGUMENT 31:8",
        "DUPLICATE_FIELD_NAME 32:27",
        "INVALID_FIELD_NAME 32:8",
        "INVALID_FIELD_NAME 33:13",
    ];
    expected.sort();
    pretty_assertions::assert_eq!(actual, expected);
}

#[test]
fn part_file_sees_the_imports_of_its_library() {
    let main = "import 'dart:async';\npart 'part.dart';\nclass Base {}\n";
    let part =
        "part of 'main.dart';\nclass A extends Base {\n  int foo = 1;\n  Future<int>? f;\n}\n";
    let Some(a) = run(&[("main.dart", main), ("part.dart", part)]) else {
        return;
    };
    let part_unit = &a.library.units[1];
    let names = support::diagnostic_names(&part_unit.diagnostics);
    assert!(names.is_empty(), "{names:?}");
    let ast = &part_unit.ast;
    let int_type = support::find_node(
        ast,
        part_unit.unit.raw(),
        NodeKind::NamedType,
        part.find("int foo").unwrap() as u32,
    )
    .unwrap();
    let t = *part_unit.tables.annotation_type.get(int_type).unwrap();
    let ctx = a.ctx(part_unit);
    assert_eq!(
        dartr_element::type_display_string_with(&ctx, t, Default::default()),
        "int"
    );
}

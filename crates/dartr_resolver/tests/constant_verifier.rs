//! Ports of the analyzer diagnostic tests of the codes that the constant
//! code reports (`pkg/analyzer/test/src/diagnostics/*_test.dart`): every
//! test that is a single `resolveTestCodeWithDiagnostics` call.
//!
//! The inline expectation markers of the Dart tests (`// ^^^` caret lines,
//! `// [diag.name]` and `// [context N]` lines, see
//! `pkg/analyzer_testing/lib/src/expected_diagnostics.dart`) are parsed here
//! and removed from the code before the analysis, like Dart does. The
//! comparison is limited to the codes of `tools/difftest/constant_codes.txt`
//! (the codes of the constant verifier and evaluator), on both sides: the
//! other verifiers are not ported yet. Each test asserts the exact sorted
//! list of (code, offset, length).

mod support;

use std::collections::BTreeSet;

/// One expected or actual diagnostic: (code, offset, length).
type Entry = (String, usize, usize);

/// The codes compared (`DiagnosticCode.lowerCaseName`).
fn constant_codes() -> BTreeSet<String> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tools/difftest/constant_codes.txt"
    );
    std::fs::read_to_string(path)
        .expect("constant_codes.txt")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect()
}

fn is_caret_line(text: &str) -> bool {
    let t = text.trim_start_matches([' ', '\t']);
    let Some(rest) = t.strip_prefix("//") else {
        return false;
    };
    let rest = rest.trim_matches([' ', '\t']);
    !rest.is_empty() && rest.chars().all(|c| c == '^')
}

fn is_expectation_line(text: &str) -> bool {
    let t = text.trim_start_matches([' ', '\t']);
    let Some(rest) = t.strip_prefix("//") else {
        return false;
    };
    let rest = rest.trim_start_matches([' ', '\t']);
    rest.starts_with("[diag.") || rest.starts_with("[context ")
}

/// The `diag.<constantName>` of a code: the camel case unique name.
fn code_for_constant_name(name: &str) -> &'static dartr_diagnostics::DiagnosticCode {
    dartr_diagnostics::all_codes()
        .iter()
        .copied()
        .find(|c| c.camel_case_name == name)
        .unwrap_or_else(|| panic!("unknown diagnostic diag.{name}"))
}

/// Dart `removeDiagnosticExpectations` and the expectations of the
/// markers: (clean code, expected diagnostics).
fn parse_markers(code: &str) -> (String, Vec<Entry>) {
    // Lines with their terminators.
    let mut lines: Vec<(&str, &str)> = Vec::new();
    let mut rest = code;
    while !rest.is_empty() {
        match rest.find('\n') {
            Some(i) => {
                lines.push((&rest[..i], "\n"));
                rest = &rest[i + 1..];
            }
            None => {
                lines.push((rest, ""));
                rest = "";
            }
        }
    }
    let mut clean = String::new();
    let mut code_lines: Vec<(&str, &str)> = Vec::new();
    let mut expected = Vec::new();
    let mut line_start = 0usize;
    let mut last_code_line_start = 0usize;
    let mut caret: Option<(usize, usize)> = None;
    for &(text, terminator) in &lines {
        if is_caret_line(text) {
            let column = text.find('^').unwrap() + 1;
            let length = text.matches('^').count();
            caret = Some((column, length));
            continue;
        }
        if is_expectation_line(text) {
            let t = text
                .trim_start_matches([' ', '\t'])
                .trim_start_matches("//");
            let t = t.trim_start_matches([' ', '\t']);
            if let Some(after) = t.strip_prefix("[diag.") {
                let end = after.find(']').unwrap();
                let name = &after[..end];
                let mut location = caret;
                let tail = &after[end + 1..];
                if let Some(c) = tail.strip_prefix("[column ") {
                    let ce = c.find(']').unwrap();
                    let column: usize = c[..ce].parse().unwrap();
                    let l = c[ce + 1..].strip_prefix("[length ").unwrap();
                    let le = l.find(']').unwrap();
                    let length: usize = l[..le].parse().unwrap();
                    location = Some((column, length));
                }
                let (column, length) = location.expect("caret or explicit location");
                let code = code_for_constant_name(name);
                // Offsets count UTF-16 code units; the test sources are ASCII.
                expected.push((
                    code.lower_case_name().to_string(),
                    last_code_line_start + column - 1,
                    length,
                ));
            }
            continue;
        }
        caret = None;
        code_lines.push((text, terminator));
        last_code_line_start = line_start;
        line_start += text.len() + terminator.len();
    }
    for (i, &(text, terminator)) in code_lines.iter().enumerate() {
        clean.push_str(text);
        if i < code_lines.len() - 1 {
            clean.push_str(terminator);
        }
    }
    // Dart keeps the terminator of the last retained line only between
    // lines; the raw strings of the tests end with a newline line.
    (clean, expected)
}

/// Analyzes [code] (with expectation markers) as `main.dart` and compares
/// the diagnostics of the constant codes with the markers.
#[track_caller]
fn check(code: &str) {
    let (clean, expected) = parse_markers(code);
    let Some(a) = support::analyze(&[("main.dart", &clean)]) else {
        return;
    };
    let codes = constant_codes();
    let mut expected: Vec<Entry> = expected
        .into_iter()
        .filter(|(c, _, _)| codes.contains(c))
        .collect();
    let unit = a.unit();
    assert!(unit.panic.is_none(), "panic: {:?}", unit.panic);
    let mut actual: Vec<Entry> = unit
        .diagnostics
        .iter()
        .filter(|d| codes.contains(d.code.lower_case_name()))
        .map(|d| (d.code.lower_case_name().to_string(), d.offset, d.length))
        .collect();
    expected.sort_by(|a, b| (a.1, &a.0, a.2).cmp(&(b.1, &b.0, b.2)));
    actual.sort_by(|a, b| (a.1, &a.0, a.2).cmp(&(b.1, &b.0, b.2)));
    assert_eq!(actual, expected, "\n{clean}");
}

/// Port of `pkg/analyzer/test/src/diagnostics/const_initialized_with_non_constant_value_test.dart`.
mod const_initialized_with_non_constant_value {
    use super::check;

    #[test]
    fn dynamic() {
        check(
            r#"
f(p) {
  const c = p;
//      ^
// [diag.unusedLocalVariable] The value of the local variable 'c' isn't used.
//          ^
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
}
"#,
        );
    }

    #[test]
    fn final_field() {
        check(
            r#"
class Foo {
  final field = 0;
  foo([int x = field]) {}
//             ^^^^^
// [diag.nonConstantDefaultValue] The default value of an optional parameter must be constant.
// [diag.implicitThisReferenceInInitializer] The instance member 'field' can't be accessed in an initializer.
}
"#,
        );
    }

    #[test]
    fn function_expression() {
        check(
            r#"
const a = () {};
//        ^^^^^
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
"#,
        );
    }

    #[test]
    fn missing_const_in_list_literal() {
        check(
            r#"
const List L = [0];
"#,
        );
    }

    #[test]
    fn missing_const_in_map_literal() {
        check(
            r#"
const Map M = {'a' : 0};
"#,
        );
    }

    #[test]
    fn new_instance_const_constructor() {
        check(
            r#"
class A {
  const A();
}
const a = new A();
//        ^^^^^^^
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
"#,
        );
    }

    #[test]
    fn new_instance_external_factory_const_constructor() {
        check(
            r#"
class A {
  external const factory A();
}
const x = const A();
"#,
        );
    }

    #[test]
    fn property_extraction_target_not_const() {
        check(
            r#"
class A {
  const A();
  int m() => 0;
}
final a = const A();
const c = a.m;
//        ^
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
"#,
        );
    }

    #[test]
    fn type_literal_interface_type() {
        check(
            r#"
const a = int;
"#,
        );
    }

    #[test]
    fn type_literal_type_alias_interface_type() {
        check(
            r#"
typedef A = int;
const a = A;
"#,
        );
    }
}

/// Port of `pkg/analyzer/test/src/diagnostics/const_map_key_not_primitive_equality_test.dart`.
mod const_map_key_not_primitive_equality {
    use super::check;

    #[test]
    fn declares_eq_eq_abstract() {
        check(
            r#"
class A {
  const A();
  bool operator==(Object other);
}

main() {
  const {const A(): 0};
}
"#,
        );
    }

    #[test]
    fn implements_eq_eq_direct() {
        check(
            r#"
class A {
  const A();
  operator ==(other) => false;
}

main() {
  const {const A() : 0};
//       ^^^^^^^^^
// [diag.constMapKeyNotPrimitiveEquality] The type of a key in a constant map can't override the '==' operator, or 'hashCode', but the class 'A' does.
}
"#,
        );
    }

    #[test]
    fn implements_eq_eq_double() {
        check(
            r#"
main() {
  const {double.infinity: 0};
//       ^^^^^^^^^^^^^^^
// [diag.constMapKeyNotPrimitiveEquality] The type of a key in a constant map can't override the '==' operator, or 'hashCode', but the class 'double' does.
}
"#,
        );
    }

    #[test]
    fn implements_eq_eq_dynamic() {
        check(
            r#"
class A {
  const A();
  operator ==(other) => false;
}

class B {
  static const a = const A();
}

main() {
  const {B.a : 0};
//       ^^^
// [diag.constMapKeyNotPrimitiveEquality] The type of a key in a constant map can't override the '==' operator, or 'hashCode', but the class 'A' does.
}
"#,
        );
    }

    #[test]
    fn implements_eq_eq_factory() {
        check(
            r#"
class A {
  const factory A() = B;
}

class B implements A {
  const B();
  operator ==(o) => true;
}

main() {
  const {const A(): 42};
//       ^^^^^^^^^
// [diag.constMapKeyNotPrimitiveEquality] The type of a key in a constant map can't override the '==' operator, or 'hashCode', but the class 'B' does.
}
"#,
        );
    }

    #[test]
    fn implements_eq_eq_nested_in_instance_creation() {
        check(
            r#"
class A {
  const A();

  bool operator ==(other) => false;
}

class B {
  const B(_);
}

main() {
  const B({A(): 0});
//         ^^^
// [diag.constMapKeyNotPrimitiveEquality] The type of a key in a constant map can't override the '==' operator, or 'hashCode', but the class 'A' does.
}
"#,
        );
    }

    #[test]
    fn implements_eq_eq_record_named() {
        check(
            r#"
class A {
  const A();
  operator ==(other) => false;
}

const x = {
  (a: 0, b: const A()): 0,
//^^^^^^^^^^^^^^^^^^^^
// [diag.constMapKeyNotPrimitiveEquality] The type of a key in a constant map can't override the '==' operator, or 'hashCode', but the class '({int a, A b})' does.
};
"#,
        );
    }

    #[test]
    fn implements_eq_eq_record_positional() {
        check(
            r#"
class A {
  const A();
  operator ==(other) => false;
}

const x = {
  (0, const A()): 0,
//^^^^^^^^^^^^^^
// [diag.constMapKeyNotPrimitiveEquality] The type of a key in a constant map can't override the '==' operator, or 'hashCode', but the class '(int, A)' does.
};
"#,
        );
    }

    #[test]
    fn implements_eq_eq_super() {
        check(
            r#"
class A {
  const A();
  operator ==(other) => false;
}

class B extends A {
  const B();
}

main() {
  const {const B() : 0};
//       ^^^^^^^^^
// [diag.constMapKeyNotPrimitiveEquality] The type of a key in a constant map can't override the '==' operator, or 'hashCode', but the class 'B' does.
}
"#,
        );
    }

    #[test]
    fn implements_hash_code_direct() {
        check(
            r#"
const v = {A(): 0};
//         ^^^
// [diag.constMapKeyNotPrimitiveEquality] The type of a key in a constant map can't override the '==' operator, or 'hashCode', but the class 'A' does.

class A {
  const A();
  int get hashCode => 0;
}
"#,
        );
    }

    #[test]
    fn implements_none_record_named() {
        check(
            r#"
class A {
  const A();
}

const x = {
  (a: 0, b: const A()): 0,
};
"#,
        );
    }

    #[test]
    fn implements_none_record_positional() {
        check(
            r#"
class A {
  const A();
}

const x = {
  (0, const A()): 0,
};
"#,
        );
    }
}

/// Port of `pkg/analyzer/test/src/diagnostics/const_set_element_not_primitive_equality_test.dart`.
mod const_set_element_not_primitive_equality {
    use super::check;

    #[test]
    fn implements_eq_eq_const_field() {
        check(
            r#"
class A {
  static const a = const A();
  const A();
  operator ==(other) => false;
}
main() {
  const {A.a};
//       ^^^
// [diag.constSetElementNotPrimitiveEquality] An element in a constant set can't override the '==' operator, or 'hashCode', but the type 'A' does.
}
"#,
        );
    }

    #[test]
    fn implements_eq_eq_direct() {
        check(
            r#"
class A {
  const A();
  operator ==(other) => false;
}
main() {
  const {const A()};
//       ^^^^^^^^^
// [diag.constSetElementNotPrimitiveEquality] An element in a constant set can't override the '==' operator, or 'hashCode', but the type 'A' does.
}
"#,
        );
    }

    #[test]
    fn implements_eq_eq_dynamic() {
        check(
            r#"
class A {
  const A();
  operator ==(other) => false;
}
class B {
  static const a = const A();
}
main() {
  const {B.a};
//       ^^^
// [diag.constSetElementNotPrimitiveEquality] An element in a constant set can't override the '==' operator, or 'hashCode', but the type 'A' does.
}
"#,
        );
    }

    #[test]
    fn implements_eq_eq_factory() {
        check(
            r#"
class A { const factory A() = B; }

class B implements A {
  const B();

  operator ==(o) => true;
}

main() {
  var m = const {const A()};
//               ^^^^^^^^^
// [diag.constSetElementNotPrimitiveEquality] An element in a constant set can't override the '==' operator, or 'hashCode', but the type 'B' does.
  print(m);
}
"#,
        );
    }

    #[test]
    fn implements_eq_eq_nested_in_instance_creation() {
        check(
            r#"
class A {
  const A();

  bool operator ==(other) => false;
}

class B {
  const B(_);
}

main() {
  const B({A()});
//         ^^^
// [diag.constSetElementNotPrimitiveEquality] An element in a constant set can't override the '==' operator, or 'hashCode', but the type 'A' does.
}
"#,
        );
    }

    #[test]
    fn implements_eq_eq_record_named() {
        check(
            r#"
class A {
  const A();
  operator ==(other) => false;
}

const x = {
  (a: 0, b: const A()),
//^^^^^^^^^^^^^^^^^^^^
// [diag.constSetElementNotPrimitiveEquality] An element in a constant set can't override the '==' operator, or 'hashCode', but the type '({int a, A b})' does.
};
"#,
        );
    }

    #[test]
    fn implements_eq_eq_record_positional() {
        check(
            r#"
class A {
  const A();
  operator ==(other) => false;
}

const x = {
  (0, const A()),
//^^^^^^^^^^^^^^
// [diag.constSetElementNotPrimitiveEquality] An element in a constant set can't override the '==' operator, or 'hashCode', but the type '(int, A)' does.
};
"#,
        );
    }

    #[test]
    fn implements_eq_eq_spread_into_list_set() {
        check(
            r#"
class A {
  const A();
  operator ==(other) => false;
}

main() {
  const [...{A()}];
//           ^^^
// [diag.constSetElementNotPrimitiveEquality] An element in a constant set can't override the '==' operator, or 'hashCode', but the type 'A' does.
}
"#,
        );
    }

    #[test]
    fn implements_eq_eq_spread_into_set_list() {
        check(
            r#"
class A {
  const A();
  operator ==(other) => false;
}

main() {
  const {...[A()]};
//       ^^^^^^^^
// [diag.constSetElementNotPrimitiveEquality] An element in a constant set can't override the '==' operator, or 'hashCode', but the type 'List<A>' does.
}
"#,
        );
    }

    #[test]
    fn implements_eq_eq_spread_into_set_set() {
        check(
            r#"
class A {
  const A();
  operator ==(other) => false;
}

main() {
  const {...{A()}};
//           ^^^
// [diag.constSetElementNotPrimitiveEquality] An element in a constant set can't override the '==' operator, or 'hashCode', but the type 'A' does.
}
"#,
        );
    }

    #[test]
    fn implements_eq_eq_super() {
        check(
            r#"
class A {
  const A();
  operator ==(other) => false;
}
class B extends A {
  const B();
}
main() {
  const {const B()};
//       ^^^^^^^^^
// [diag.constSetElementNotPrimitiveEquality] An element in a constant set can't override the '==' operator, or 'hashCode', but the type 'B' does.
}
"#,
        );
    }

    #[test]
    fn implements_hash_code_direct() {
        check(
            r#"
const v = {A()};
//         ^^^
// [diag.constSetElementNotPrimitiveEquality] An element in a constant set can't override the '==' operator, or 'hashCode', but the type 'A' does.

class A {
  const A();
  int get hashCode => 0;
}
"#,
        );
    }

    #[test]
    fn implements_none_record_named() {
        check(
            r#"
class A {
  const A();
}

const x = {
  (a: 0, b: const A()): 0,
};
"#,
        );
    }

    #[test]
    fn implements_none_record_positional() {
        check(
            r#"
class A {
  const A();
}

const x = {
  (0, const A()): 0,
};
"#,
        );
    }

    #[test]
    fn list_literal_spread() {
        check(
            r#"
class A {
  const A();
  operator ==(other) => false;
}

main() {
  const [...[A()]];
}
"#,
        );
    }
}

/// Port of `pkg/analyzer/test/src/diagnostics/equal_elements_in_const_set_test.dart`.
mod equal_elements_in_const_set {
    use super::check;

    #[test]
    fn const_entry() {
        check(
            r#"
var c = const {1, 2, 1};
//             ^
// [context 1] The first element with this value.
//                   ^
// [diag.equalElementsInConstSet][context 1] Two elements in a constant set literal can't be equal.
"#,
        );
    }

    #[test]
    fn const_entry_extension_type_type_value() {
        check(
            r#"
const x = {int, E};
//         ^^^
// [context 1] The first element with this value.
//              ^
// [diag.equalElementsInConstSet][context 1] Two elements in a constant set literal can't be equal.
extension type E(int it) {}
"#,
        );
    }

    #[test]
    fn const_if_element_then_else_false() {
        check(
            r#"
var c = const {1, if (1 < 0) 2 else 1};
//             ^
// [context 1] The first element with this value.
//                                  ^
// [diag.equalElementsInConstSet][context 1] Two elements in a constant set literal can't be equal.
"#,
        );
    }

    #[test]
    fn const_if_element_then_else_false_only_else() {
        check(
            r#"
var c = const {if (0 < 1) 1 else 1};
"#,
        );
    }

    #[test]
    fn const_if_element_then_else_true() {
        check(
            r#"
var c = const {1, if (0 < 1) 2 else 1};
"#,
        );
    }

    #[test]
    fn const_if_element_then_else_true_only_then() {
        check(
            r#"
var c = const {if (0 < 1) 1 else 1};
"#,
        );
    }

    #[test]
    fn const_if_element_then_false() {
        check(
            r#"
var c = const {2, if (1 < 0) 2};
"#,
        );
    }

    #[test]
    fn const_if_element_then_true() {
        check(
            r#"
var c = const {1, if (0 < 1) 1};
//             ^
// [context 1] The first element with this value.
//                           ^
// [diag.equalElementsInConstSet][context 1] Two elements in a constant set literal can't be equal.
"#,
        );
    }

    #[test]
    fn const_instance_creation_equal_type_args() {
        check(
            r#"
class A<T> {
  const A();
}

var c = const {const A<int>(), const A<int>()};
//             ^^^^^^^^^^^^^^
// [context 1] The first element with this value.
//                             ^^^^^^^^^^^^^^
// [diag.equalElementsInConstSet][context 1] Two elements in a constant set literal can't be equal.
"#,
        );
    }

    #[test]
    fn const_instance_creation_not_equal_type_args() {
        check(
            r#"
class A<T> {
  const A();
}

var c = const {const A<int>(), const A<num>()};
"#,
        );
    }

    #[test]
    fn const_list_has_equal() {
        check(
            r#"
const x = {[0], [0]};
//         ^^^
// [context 1] The first element with this value.
//              ^^^
// [diag.equalElementsInConstSet][context 1] Two elements in a constant set literal can't be equal.
"#,
        );
    }

    #[test]
    fn const_list_no_equal() {
        check(
            r#"
const x = {[0], [1]};
"#,
        );
    }

    #[test]
    fn const_record_has_equal() {
        check(
            r#"
const x = {(0, 1), (0, 1)};
//         ^^^^^^
// [context 1] The first element with this value.
//                 ^^^^^^
// [diag.equalElementsInConstSet][context 1] Two elements in a constant set literal can't be equal.
"#,
        );
    }

    #[test]
    fn const_record_no_equal() {
        check(
            r#"
const x = {(0, 1), (0, 2)};
"#,
        );
    }

    #[test]
    fn const_spread_no_duplicate() {
        check(
            r#"
var c = const {1, ...{2}};
"#,
        );
    }

    #[test]
    fn const_spread_has_duplicate() {
        check(
            r#"
var c = const {1, ...{1}};
//             ^
// [context 1] The first element with this value.
//                   ^^^
// [diag.equalElementsInConstSet][context 1] Two elements in a constant set literal can't be equal.
"#,
        );
    }

    #[test]
    fn non_const_entry() {
        check(
            r#"
var c = {1, 2, 1};
//             ^
// [diag.equalElementsInSet] Two elements in a set literal shouldn't be equal.
"#,
        );
    }
}

/// Port of `pkg/analyzer/test/src/diagnostics/equal_keys_in_const_map_test.dart`.
mod equal_keys_in_const_map {
    use super::check;

    #[test]
    fn const_entry() {
        check(
            r#"
var c = const {1: null, 2: null, 1: null};
//             ^
// [context 1] The first key with this value.
//                               ^
// [diag.equalKeysInConstMap][context 1] Two keys in a constant map literal can't be equal.
"#,
        );
    }

    #[test]
    fn const_entry_extension_type_type_value() {
        check(
            r#"
const x = {int: 0, E: 0};
//         ^^^
// [context 1] The first key with this value.
//                 ^
// [diag.equalKeysInConstMap][context 1] Two keys in a constant map literal can't be equal.
extension type E(int it) {}
"#,
        );
    }

    #[test]
    fn const_if_element_then_else_false() {
        check(
            r#"
var c = const {1: null, if (1 < 0) 2: null else 1: null};
//             ^
// [context 1] The first key with this value.
//                                              ^
// [diag.equalKeysInConstMap][context 1] Two keys in a constant map literal can't be equal.
"#,
        );
    }

    #[test]
    fn const_if_element_then_else_false_only_else() {
        check(
            r#"
var c = const {if (0 < 1) 1: null else 1: null};
"#,
        );
    }

    #[test]
    fn const_if_element_then_else_true() {
        check(
            r#"
var c = const {1: null, if (0 < 1) 2: null else 1: null};
"#,
        );
    }

    #[test]
    fn const_if_element_then_else_true_only_then() {
        check(
            r#"
var c = const {if (0 < 1) 1: null else 1: null};
"#,
        );
    }

    #[test]
    fn const_if_element_then_false() {
        check(
            r#"
var c = const {2: null, if (1 < 0) 2: 2};
"#,
        );
    }

    #[test]
    fn const_if_element_then_true() {
        check(
            r#"
var c = const {1: null, if (0 < 1) 1: null};
//             ^
// [context 1] The first key with this value.
//                                 ^
// [diag.equalKeysInConstMap][context 1] Two keys in a constant map literal can't be equal.
"#,
        );
    }

    #[test]
    fn const_instance_creation_equal_type_args() {
        check(
            r#"
class A<T> {
  const A();
}

var c = const {const A<int>(): null, const A<int>(): null};
//             ^^^^^^^^^^^^^^
// [context 1] The first key with this value.
//                                   ^^^^^^^^^^^^^^
// [diag.equalKeysInConstMap][context 1] Two keys in a constant map literal can't be equal.
"#,
        );
    }

    #[test]
    fn const_instance_creation_not_equal_type_args() {
        check(
            r#"
class A<T> {
  const A();
}

var c = const {const A<int>(): null, const A<num>(): null};
"#,
        );
    }

    #[test]
    fn const_list_has_equal() {
        check(
            r#"
const x = {[0]: null, [0]: null};
//         ^^^
// [context 1] The first key with this value.
//                    ^^^
// [diag.equalKeysInConstMap][context 1] Two keys in a constant map literal can't be equal.
"#,
        );
    }

    #[test]
    fn const_list_no_equal() {
        check(
            r#"
const x = {[0]: null, [1]: null};
"#,
        );
    }

    #[test]
    fn const_record_has_equal() {
        check(
            r#"
const x = {(0, 1): null, (0, 1): null};
//         ^^^^^^
// [context 1] The first key with this value.
//                       ^^^^^^
// [diag.equalKeysInConstMap][context 1] Two keys in a constant map literal can't be equal.
"#,
        );
    }

    #[test]
    fn const_record_no_equal() {
        check(
            r#"
const x = {(0, 1): null, (0, 2): null};
"#,
        );
    }

    #[test]
    fn const_spread_no_duplicate() {
        check(
            r#"
var c = const {1: null, ...{2: null}};
"#,
        );
    }

    #[test]
    fn const_spread_has_duplicate() {
        check(
            r#"
var c = const {1: null, ...{1: null}};
//             ^
// [context 1] The first key with this value.
//                         ^^^^^^^^^
// [diag.equalKeysInConstMap][context 1] Two keys in a constant map literal can't be equal.
"#,
        );
    }

    #[test]
    fn non_const_entry() {
        check(
            r#"
var c = {1: null, 2: null, 1: null};
//                         ^
// [diag.equalKeysInMap] Two keys in a map literal shouldn't be equal.
"#,
        );
    }
}

/// Port of `pkg/analyzer/test/src/diagnostics/equal_keys_in_map_pattern_test.dart`.
mod equal_keys_in_map_pattern {
    use super::check;

    #[test]
    fn identical_double() {
        check(
            r#"
void f(x) {
  if (x case {3.14: 1, 3.14: 2}) {}
//            ^^^^
// [context 1] The first key with this value.
//                     ^^^^
// [diag.equalKeysInMapPattern][context 1] Two keys in a map pattern can't be equal.
}
"#,
        );
    }

    #[test]
    fn identical_int() {
        check(
            r#"
void f(x) {
  if (x case {0: 1, 0: 2}) {}
//            ^
// [context 1] The first key with this value.
//                  ^
// [diag.equalKeysInMapPattern][context 1] Two keys in a map pattern can't be equal.
}
"#,
        );
    }

    #[test]
    fn identical_int_via_identifier() {
        check(
            r#"
const a = 0;
const b = 0;

void f(x) {
  if (x case {a: 1, b: 2}) {}
//            ^
// [context 1] The first key with this value.
//                  ^
// [diag.equalKeysInMapPattern][context 1] Two keys in a map pattern can't be equal.
}
"#,
        );
    }

    #[test]
    fn identical_type() {
        check(
            r#"
void f(x) {
  if (x case {int: 0, int: 0}) {}
//            ^^^
// [context 1] The first key with this value.
//                    ^^^
// [diag.equalKeysInMapPattern][context 1] Two keys in a map pattern can't be equal.
}
"#,
        );
    }

    #[test]
    fn identical_type_extension_type() {
        check(
            r#"
void f(x) {
  if (x case {int: 0, E: 0}) {}
//            ^^^
// [context 1] The first key with this value.
//                    ^
// [diag.equalKeysInMapPattern][context 1] Two keys in a map pattern can't be equal.
}
extension type E(int it) {}
"#,
        );
    }

    #[test]
    fn not_identical_double() {
        check(
            r#"
void f(x) {
  if (x case {3.14: 1, 2.71: 2}) {}
}
"#,
        );
    }

    #[test]
    fn not_identical_int() {
        check(
            r#"
void f(x) {
  if (x case {0: 1, 2: 3}) {}
}
"#,
        );
    }

    #[test]
    fn not_identical_user_class() {
        check(
            r#"
void f(x) {
  if (x case {const A(0): 1, const A(2): 3}) {}
}

class A {
  final int field;
  const A(this.field);
  bool operator ==(other) => false;
}
"#,
        );
    }

    #[test]
    fn record_type_not_primitive_equal_named() {
        check(
            r#"
void f(x) {
  if (x case {(a: const A()): 1, (a: const A()): 2}) {}
}

class A {
  const A();
  bool operator ==(other) => true;
}
"#,
        );
    }

    #[test]
    fn record_type_not_primitive_equal_positional() {
        check(
            r#"
void f(x) {
  if (x case {(0, const A()): 1, (0, const A()): 2}) {}
}

class A {
  const A();
  bool operator ==(other) => true;
}
"#,
        );
    }

    #[test]
    fn record_type_primitive_equal_different_shape() {
        check(
            r#"
void f(x) {
  if (x case {(0, 1): 2, (0,): 3}) {}
}
"#,
        );
    }

    #[test]
    fn record_type_primitive_equal_empty() {
        check(
            r#"
void f(x) {
  if (x case {(): 1, (): 2}) {}
//            ^^
// [context 1] The first key with this value.
//                   ^^
// [diag.equalKeysInMapPattern][context 1] Two keys in a map pattern can't be equal.
}
"#,
        );
    }

    #[test]
    fn record_type_primitive_equal_named_equal() {
        check(
            r#"
void f(x) {
  if (x case {(a: 0): 1, (a: 0): 2}) {}
//            ^^^^^^
// [context 1] The first key with this value.
//                       ^^^^^^
// [diag.equalKeysInMapPattern][context 1] Two keys in a map pattern can't be equal.
}
"#,
        );
    }

    #[test]
    fn record_type_primitive_equal_named_not_equal() {
        check(
            r#"
void f(x) {
  if (x case {(a: 0): 1, (a: 2): 3}) {}
}
"#,
        );
    }

    #[test]
    fn record_type_primitive_equal_positional_equal() {
        check(
            r#"
void f(x) {
  if (x case {(0,): 1, (0,): 2}) {}
//            ^^^^
// [context 1] The first key with this value.
//                     ^^^^
// [diag.equalKeysInMapPattern][context 1] Two keys in a map pattern can't be equal.
}
"#,
        );
    }

    #[test]
    fn record_type_primitive_equal_positional_not_equal() {
        check(
            r#"
void f(x) {
  if (x case {(0,): 1, (2,): 3}) {}
}
"#,
        );
    }
}

/// Port of `pkg/analyzer/test/src/diagnostics/non_constant_list_element_test.dart`.
mod non_constant_list_element {
    use super::check;

    #[test]
    fn const_top_var_nested() {
        check(
            r#"
final dynamic a = 0;
var v = const [a + 1];
//             ^
// [diag.nonConstantListElement] The values in a const list literal must be constants.
"#,
        );
    }
}

/// Port of `pkg/analyzer/test/src/diagnostics/non_constant_set_element_test.dart`.
mod non_constant_set_element {
    use super::check;

    #[test]
    fn const_parameter() {
        check(
            r#"
f(a) {
  return const {a};
//              ^
// [diag.nonConstantSetElement] The values in a const set literal must be constants.
}"#,
        );
    }

    #[test]
    fn const_spread_final() {
        check(
            r#"
final Set x = {};
var v = const {...x};
//                ^
// [diag.nonConstantSetElement] The values in a const set literal must be constants.
"#,
        );
    }
}

/// Port of `pkg/analyzer/test/src/diagnostics/non_constant_map_key_test.dart`.
mod non_constant_map_key {
    use super::check;

    #[test]
    fn const_if_element_then_true_else_final() {
        check(
            r#"
final dynamic a = 0;
const cond = true;
var v = const {if (cond) 0: 1 else a : 0};
//                                 ^
// [diag.nonConstantMapKey] The keys in a const map literal must be constant.
"#,
        );
    }

    #[test]
    fn const_if_element_then_true_then_final() {
        check(
            r#"
final dynamic a = 0;
const cond = true;
var v = const {if (cond) a : 0};
//                       ^
// [diag.nonConstantMapKey] The keys in a const map literal must be constant.
"#,
        );
    }

    #[test]
    fn const_top_level() {
        check(
            r#"
final dynamic a = 0;
var v = const {a : 0};
//             ^
// [diag.nonConstantMapKey] The keys in a const map literal must be constant.
"#,
        );
    }
}

/// Port of `pkg/analyzer/test/src/diagnostics/non_constant_map_value_test.dart`.
mod non_constant_map_value {
    use super::check;

    #[test]
    fn const_if_true_else_final() {
        check(
            r#"
final dynamic a = 0;
const cond = true;
var v = const {if (cond) 'a': 'b', 'c' : a};
//                                       ^
// [diag.nonConstantMapValue] The values in a const map literal must be constant.
"#,
        );
    }

    #[test]
    fn const_if_true_then_final() {
        check(
            r#"
final dynamic a = 0;
const cond = true;
var v = const {if (cond) 'a' : a};
//                             ^
// [diag.nonConstantMapValue] The values in a const map literal must be constant.
"#,
        );
    }

    #[test]
    fn const_top_level() {
        check(
            r#"
final dynamic a = 0;
var v = const {'a' : a};
//                   ^
// [diag.nonConstantMapValue] The values in a const map literal must be constant.
"#,
        );
    }
}

/// Port of `pkg/analyzer/test/src/diagnostics/non_constant_default_value_test.dart`.
mod non_constant_default_value {
    use super::check;

    #[test]
    fn constructor_named() {
        check(
            r#"
class A {
  int y = 0;
  A({x = y}) {}
//       ^
// [diag.nonConstantDefaultValue] The default value of an optional parameter must be constant.
// [diag.implicitThisReferenceInInitializer] The instance member 'y' can't be accessed in an initializer.
}
"#,
        );
    }

    #[test]
    fn constructor_positional() {
        check(
            r#"
class A {
  int y = 0;
  A([x = y]) {}
//       ^
// [diag.nonConstantDefaultValue] The default value of an optional parameter must be constant.
// [diag.implicitThisReferenceInInitializer] The instance member 'y' can't be accessed in an initializer.
}
"#,
        );
    }

    #[test]
    fn dot_shorthand_issue60962() {
        check(
            r#"
class A {
  const A();
}

void f([A a = .new()]) {}
//            ^^^^^^
// [diag.nonConstantDefaultValue] The default value of an optional parameter must be constant.
"#,
        );
    }

    #[test]
    fn function_named() {
        check(
            r#"
int y = 0;
f({x = y}) {}
//     ^
// [diag.nonConstantDefaultValue] The default value of an optional parameter must be constant.
"#,
        );
    }

    #[test]
    fn function_named_const_list() {
        check(
            r#"
void f({x = const [0, 1]}) {}
"#,
        );
    }

    #[test]
    fn function_named_const_list_elements_list_literal() {
        check(
            r#"
void f({x = const [0, [1]]}) {}
"#,
        );
    }

    #[test]
    fn function_named_const_record() {
        check(
            r#"
void f({x = const (0, 1)}) {}
"#,
        );
    }

    #[test]
    fn function_named_const_record_named_fields_list_literal() {
        check(
            r#"
void f({x = const (0, foo: [1])}) {}
"#,
        );
    }

    #[test]
    fn function_named_const_record_positional_fields_list_literal() {
        check(
            r#"
void f({x = const (0, [1])}) {}
"#,
        );
    }

    #[test]
    fn function_named_record_named_fields_integer_literal() {
        check(
            r#"
void f({x = (a: 0, b: 1)}) {}
"#,
        );
    }

    #[test]
    fn function_named_record_named_fields_list_literal() {
        check(
            r#"
void f({x = (a: 0, b: [1])}) {}
//                    ^^^
// [diag.nonConstantDefaultValue] The default value of an optional parameter must be constant.
"#,
        );
    }

    #[test]
    fn function_named_record_named_fields_list_literal_const() {
        check(
            r#"
void f({x = (a: 0, b: const [1])}) {}
"#,
        );
    }

    #[test]
    fn function_named_record_positional_fields_integer_literal() {
        check(
            r#"
void f({x = (0, 1)}) {}
"#,
        );
    }

    #[test]
    fn function_named_record_positional_fields_list_literal() {
        check(
            r#"
void f({x = (0, [1])}) {}
//              ^^^
// [diag.nonConstantDefaultValue] The default value of an optional parameter must be constant.
"#,
        );
    }

    #[test]
    fn function_named_record_positional_fields_list_literal_const() {
        check(
            r#"
void f({x = (0, const [1])}) {}
"#,
        );
    }

    #[test]
    fn function_named_undefined_identifier() {
        check(
            r#"
void f({int x = X}) {}
//              ^
// [diag.undefinedIdentifier] Undefined name 'X'.
"#,
        );
    }

    #[test]
    fn function_positional() {
        check(
            r#"
int y = 0;
f([x = y]) {}
//     ^
// [diag.nonConstantDefaultValue] The default value of an optional parameter must be constant.
"#,
        );
    }

    #[test]
    fn function_positional_undefined_identifier() {
        check(
            r#"
void f([int x = X]) {}
//              ^
// [diag.undefinedIdentifier] Undefined name 'X'.
"#,
        );
    }

    #[test]
    fn method_named() {
        check(
            r#"
class A {
  int y = 0;
  m({x = y}) {}
//       ^
// [diag.nonConstantDefaultValue] The default value of an optional parameter must be constant.
// [diag.implicitThisReferenceInInitializer] The instance member 'y' can't be accessed in an initializer.
}
"#,
        );
    }

    #[test]
    fn method_positional() {
        check(
            r#"
class A {
  int y = 0;
  m([x = y]) {}
//       ^
// [diag.nonConstantDefaultValue] The default value of an optional parameter must be constant.
// [diag.implicitThisReferenceInInitializer] The instance member 'y' can't be accessed in an initializer.
}
"#,
        );
    }

    #[test]
    fn no_applied_type_parameters_default_constructor_value_dynamic() {
        check(
            r#"
void f<T>(T t) => t;

class C<T> {
  final dynamic p;
  const C({this.p = f});
}
"#,
        );
    }

    #[test]
    fn no_applied_type_parameters_default_constructor_value_generic_fn() {
        check(
            r#"
void f<T>(T t) => t;

class C<T> {
  final void Function<T>(T) p;
  const C({this.p = f});
}
"#,
        );
    }

    #[test]
    fn no_applied_type_parameters_default_function_value_generic_fn() {
        check(
            r#"
void f<T>(T t) => t;

void bar<T>([void Function<T>(T) p = f]) {}
"#,
        );
    }

    #[test]
    fn no_applied_type_parameters_default_method_value_generic_fn() {
        check(
            r#"
void f<T>(T t) => t;

class C<T> {
  void foo([void Function<T>(T) p = f]) {}
}
"#,
        );
    }

    #[test]
    fn primary_constructor_optional_named() {
        check(
            r#"
int y = 0;
class A({int x = y});
//               ^
// [diag.nonConstantDefaultValue] The default value of an optional parameter must be constant.
"#,
        );
    }

    #[test]
    fn primary_constructor_optional_positional() {
        check(
            r#"
int y = 0;
class A([int x = y]);
//               ^
// [diag.nonConstantDefaultValue] The default value of an optional parameter must be constant.
"#,
        );
    }
}

/// Port of `pkg/analyzer/test/src/diagnostics/non_constant_relational_pattern_expression_test.dart`.
mod non_constant_relational_pattern_expression {
    use super::check;

    #[test]
    fn const_integer_literal() {
        check(
            r#"
void f(x) {
  if (x case > 0) {}
}
"#,
        );
    }

    #[test]
    fn const_local_variable() {
        check(
            r#"
void f(x) {
  const a = 0;
  if (x case > a) {}
}
"#,
        );
    }

    #[test]
    fn const_top_level_variable() {
        check(
            r#"
const a = 0;

void f(x) {
  if (x case > a) {}
}
"#,
        );
    }

    #[test]
    fn not_const_formal_parameter() {
        check(
            r#"
void f(x, int a) {
  if (x case > a) {}
//             ^
// [diag.nonConstantRelationalPatternExpression] The relational pattern expression must be a constant.
}
"#,
        );
    }

    #[test]
    fn not_const_top_level_variable() {
        check(
            r#"
final a = 0;

void f(x) {
  if (x case > a) {}
//             ^
// [diag.nonConstantRelationalPatternExpression] The relational pattern expression must be a constant.
}
"#,
        );
    }
}

/// Port of `pkg/analyzer/test/src/diagnostics/non_constant_map_pattern_key_test.dart`.
mod non_constant_map_pattern_key {
    use super::check;

    #[test]
    fn formal_parameter() {
        check(
            r#"
void f(x, int a) {
  if (x case {a: 0}) {}
//            ^
// [diag.nonConstantMapPatternKey] Key expressions in map patterns must be constants.
}
"#,
        );
    }

    #[test]
    fn instance_creation_no_const() {
        check(
            r#"
void f(x) {
  if (x case {A(): 0}) {}
//            ^^^
// [diag.nonConstantMapPatternKey] Key expressions in map patterns must be constants.
}

class A {
  const A();
}
"#,
        );
    }

    #[test]
    fn integer_literal() {
        check(
            r#"
void f(x) {
  if (x case {0: 1}) {}
}
"#,
        );
    }
}

/// Port of `pkg/analyzer/test/src/diagnostics/invalid_constant_test.dart`.
mod invalid_constant {
    use super::check;

    #[test]
    fn conditional_expression_unknown_condition() {
        check(
            r#"
const bool kIsWeb = identical(0, 0.0);

void f() {
  const A(kIsWeb ? 0 : 1);
}

class A {
  const A(int _);
}
"#,
        );
    }

    #[test]
    fn conditional_expression_unknown_condition_error_in_branch() {
        check(
            r#"
const bool kIsWeb = identical(0, 0.0);

void f() {
  var x = 2;
  const A(kIsWeb ? 0 : x);
//                     ^
// [diag.invalidConstant] Invalid constant value.
}

class A {
  const A(int _);
}
"#,
        );
    }

    #[test]
    fn in_initializer_assert_condition() {
        check(
            r#"
class A {
  const A(int i) : assert(i.isNegative);
//                        ^^^^^^^^^^^^
// [diag.invalidConstant] Invalid constant value.
}
"#,
        );
    }

    #[test]
    fn in_initializer_assert_message() {
        check(
            r#"
class A {
  const A(int i) : assert(i < 0, 'isNegative = ${i.isNegative}');
//                                               ^^^^^^^^^^^^
// [diag.invalidConstant] Invalid constant value.
}
"#,
        );
    }

    #[test]
    fn in_initializer_field() {
        check(
            r#"
class A {
  static int C = 0;
  final int a;
  const A() : a = C;
//                ^
// [diag.invalidConstant] Invalid constant value.
}
"#,
        );
    }

    #[test]
    fn in_initializer_field_as() {
        check(
            r#"
class C<T> {
  final l;
  const C.test(dynamic x) : l = x as List<T>;
}
"#,
        );
    }

    #[test]
    fn in_initializer_instance_creation() {
        check(
            r#"
class A {
  A();
}
class B {
  const B() : a = new A();
//                ^^^^^^^
// [context 1] The error is in the field initializer of 'B', and occurs here.
// [diag.invalidConstant] Invalid constant value.
  final a;
}
var b = const B();
//      ^^^^^^^^^
// [diag.invalidConstant][context 1] Invalid constant value.
"#,
        );
    }

    #[test]
    fn in_initializer_redirecting() {
        check(
            r#"
class A {
  static var C;
  const A.named(p);
  const A() : this.named(C);
//                       ^
// [diag.invalidConstant] Invalid constant value.
}
"#,
        );
    }

    #[test]
    fn in_initializer_super() {
        check(
            r#"
class A {
  const A(p);
}
class B extends A {
  static var C;
  const B() : super(C);
//                  ^
// [diag.invalidConstant] Invalid constant value.
}
"#,
        );
    }

    #[test]
    fn issue49389() {
        check(
            r#"
class Foo {
  const Foo({required this.bar});
  final Map<String, String> bar;
}

void main() {
  final data = <String, String>{};
  const Foo(bar: data);
//               ^^^^
// [diag.invalidConstant] Invalid constant value.
}
"#,
        );
    }
}

/// Port of `pkg/analyzer/test/src/diagnostics/const_constructor_with_field_initialized_by_non_const_test.dart`.
mod const_constructor_with_field_initialized_by_non_const {
    use super::check;

    #[test]
    fn class_factory_constructor() {
        check(
            r#"
class A {
  final List<int> list = f();
  const factory A() = B;
}
class B implements A {
  final List<int> list = const [];
  const B();
}
List<int> f() {
  return [3];
}
"#,
        );
    }

    #[test]
    fn class_instance_field() {
        check(
            r#"
class A {
  final int i = f();
//              ^^^
// [diag.constEvalMethodInvocation] Methods can't be invoked in constant expressions.
  const A();
//^^^^^
// [diag.constConstructorWithFieldInitializedByNonConst] Can't define the 'const' constructor because the field 'i' is initialized with a non-constant value.
}
int f() {
  return 3;
}
"#,
        );
    }

    #[test]
    fn class_instance_field_as_expression() {
        check(
            r#"
dynamic y = 2;
class A {
  const A();
//^^^^^
// [diag.constConstructorWithFieldInitializedByNonConst] Can't define the 'const' constructor because the field 'x' is initialized with a non-constant value.
  final x = y as num;
}
"#,
        );
    }

    #[test]
    fn class_static_field() {
        check(
            r#"
class A {
  static final int i = f();
  const A();
}
int f() {
  return 3;
}
"#,
        );
    }

    #[test]
    fn enum_instance_field() {
        check(
            r#"
enum E {
  v;
  final int i = f();
//              ^^^
// [diag.constEvalMethodInvocation] Methods can't be invoked in constant expressions.
  const E();
//^^^^^
// [diag.constConstructorWithFieldInitializedByNonConst] Can't define the 'const' constructor because the field 'i' is initialized with a non-constant value.
}
int f() => 0;
"#,
        );
    }

    #[test]
    fn enum_static_field() {
        check(
            r#"
enum E {
  v;
  static final int i = f();
  const E();
}
int f() => 0;
"#,
        );
    }

    #[test]
    fn mixin_class_factory() {
        check(
            r#"
int e = 3;
mixin class MixinClassFactory {
  final int foo = e;
  const factory MixinClassFactory.x() = A;
}

mixin class A implements MixinClassFactory {
  @override
  final int foo = 0;
  const A();
}
"#,
        );
    }
}

/// Port of `pkg/analyzer/test/src/diagnostics/const_constructor_field_type_mismatch_test.dart`.
mod const_constructor_field_type_mismatch {
    use super::check;

    #[test]
    fn generic_int_int() {
        check(
            r#"
class C<T> {
  final T x = y;
//            ^
// [diag.invalidAssignment] A value of type 'int' can't be assigned to a variable of type 'T'.
  const C();
}
const int y = 1;
var v = const C<int>();
"#,
        );
    }

    #[test]
    fn not_generic_unresolved_int() {
        check(
            r#"
class A {
  const A(x) : y = x;
  final Unresolved y;
//      ^^^^^^^^^^
// [diag.undefinedClass] Undefined class 'Unresolved'.
}
var v = const A(0);
"#,
        );
    }

    #[test]
    fn not_generic_unresolved_null() {
        check(
            r#"
class A {
  const A(x) : y = x;
  final Unresolved y;
//      ^^^^^^^^^^
// [diag.undefinedClass] Undefined class 'Unresolved'.
}
var v = const A(null);
"#,
        );
    }
}

/// Port of `pkg/analyzer/test/src/diagnostics/const_constructor_param_type_mismatch_test.dart`.
mod const_constructor_param_type_mismatch {
    use super::check;

    #[test]
    fn assignable_field_formal_omitted_type() {
        check(
            r#"
class A {
  final x;
  const A(this.x);
}
var v = const A(5);
"#,
        );
    }

    #[test]
    fn assignable_field_formal_subtype() {
        check(
            r#"
class A {
  const A();
}
class B extends A {
  const B();
}
class C {
  final A a;
  const C(this.a);
}
var v = const C(const B());
"#,
        );
    }

    #[test]
    fn assignable_field_formal_typedef() {
        check(
            r#"
typedef String Int2String(int x);
class A {
  final Int2String f;
  const A(this.f);
}
foo(x) => 1;
var v = const A(foo);
//              ^^^
// [diag.argumentTypeNotAssignable] The argument type 'dynamic Function(dynamic)' can't be assigned to the parameter type 'Int2String'.
// [diag.constConstructorParamTypeMismatch] A value of type 'dynamic Function(dynamic)' can't be assigned to a parameter of type 'String Function(int)' in a const constructor.
"#,
        );
    }

    #[test]
    fn assignable_field_formal_type_substitution() {
        check(
            r#"
class A<T> {
  final T x;
  const A(this.x);
}
var v = const A<int>(3);
"#,
        );
    }

    #[test]
    fn assignable_type_substitution() {
        check(
            r#"
class A<T> {
  const A(T x);
}
var v = const A<int>(3);
"#,
        );
    }

    #[test]
    fn assignable_undefined() {
        check(
            r#"
class A {
  const A(Unresolved x);
//        ^^^^^^^^^^
// [diag.undefinedClass] Undefined class 'Unresolved'.
}
var v = const A('foo');
"#,
        );
    }

    #[test]
    fn assignable_undefined_null() {
        check(
            r#"
class A {
  const A(Unresolved x);
//        ^^^^^^^^^^
// [diag.undefinedClass] Undefined class 'Unresolved'.
}
var v = const A(null);
"#,
        );
    }

    #[test]
    fn not_assignable_field_formal_optional() {
        check(
            r#"
class A {
  final int x;
  const A([this.x = 'foo']);
//                  ^^^^^
// [diag.invalidAssignment] A value of type 'String' can't be assigned to a variable of type 'int'.
}
var v = const A();
//      ^^^^^^^^^
// [diag.constConstructorParamTypeMismatch] A value of type 'String' can't be assigned to a parameter of type 'int' in a const constructor.
"#,
        );
    }

    #[test]
    fn not_assignable_field_formal_supertype() {
        check(
            r#"
class A {
  const A();
}
class B extends A {
  const B();
}
class C {
  final B b;
  const C(this.b);
}
const A u = const A();
var v = const C(u);
//              ^
// [diag.argumentTypeNotAssignable] The argument type 'A' can't be assigned to the parameter type 'B'.
// [diag.constConstructorParamTypeMismatch] A value of type 'A' can't be assigned to a parameter of type 'B' in a const constructor.
"#,
        );
    }

    #[test]
    fn not_assignable_field_formal_typedef() {
        check(
            r#"
typedef String Int2String(int x);
class A {
  final Int2String f;
  const A(this.f);
}
int foo(String x) => 1;
var v = const A(foo);
//              ^^^
// [diag.argumentTypeNotAssignable] The argument type 'int Function(String)' can't be assigned to the parameter type 'Int2String'.
// [diag.constConstructorParamTypeMismatch] A value of type 'int Function(String)' can't be assigned to a parameter of type 'String Function(int)' in a const constructor.
"#,
        );
    }

    #[test]
    fn not_assignable_field_formal_unrelated() {
        check(
            r#"
class A {
  final int x;
  const A(this.x);
}
var v = const A('foo');
//              ^^^^^
// [diag.argumentTypeNotAssignable] The argument type 'String' can't be assigned to the parameter type 'int'.
// [diag.constConstructorParamTypeMismatch] A value of type 'String' can't be assigned to a parameter of type 'int' in a const constructor.
"#,
        );
    }

    #[test]
    fn not_assignable_field_formal_unresolved() {
        check(
            r#"
class A {
  final Unresolved x;
//      ^^^^^^^^^^
// [diag.undefinedClass] Undefined class 'Unresolved'.
  const A(String this.x);
}
var v = const A('foo');
"#,
        );
    }

    #[test]
    fn not_assignable_type_substitution() {
        check(
            r#"
class A<T> {
  const A(T x);
}
var v = const A<int>('foo');
//                   ^^^^^
// [diag.argumentTypeNotAssignable] The argument type 'String' can't be assigned to the parameter type 'int'.
// [diag.constConstructorParamTypeMismatch] A value of type 'String' can't be assigned to a parameter of type 'int' in a const constructor.
"#,
        );
    }

    #[test]
    fn not_assignable_unrelated() {
        check(
            r#"
class A {
  const A(int x);
}
var v = const A('foo');
//              ^^^^^
// [diag.argumentTypeNotAssignable] The argument type 'String' can't be assigned to the parameter type 'int'.
// [diag.constConstructorParamTypeMismatch] A value of type 'String' can't be assigned to a parameter of type 'int' in a const constructor.
"#,
        );
    }

    #[test]
    fn super_formal_parameter_explicit() {
        check(
            r#"
class A {
  const A({int a = 0});
}

class B extends A {
  static const f = B();

  const B({super.a = 2});
}
"#,
        );
    }

    #[test]
    fn super_formal_parameter_inherited() {
        check(
            r#"
class A {
  const A({int a = 0});
}

class B extends A {
  const B({super.a});
}

const b = const B();
"#,
        );
    }

    #[test]
    fn super_formal_parameter_inherited_generic() {
        check(
            r#"
class A<T> {
  const A({int a = 0});
}

class B extends A<int> {
  const B({super.a});
}

const b = const B();
"#,
        );
    }
}

/// Port of `pkg/analyzer/test/src/diagnostics/const_with_non_constant_argument_test.dart`.
mod const_with_non_constant_argument {
    use super::check;

    #[test]
    fn annotation() {
        check(
            r#"
class A {
  const A(int p);
}
var v = 42;
@A(v)
// ^
// [diag.constWithNonConstantArgument] Arguments of a constant creation must be constant expressions.
main() {
}
"#,
        );
    }

    #[test]
    fn class_shadowed_by_setter() {
        check(
            r#"
class Annotation {
  const Annotation(Object obj);
}

class Bar {}

class Foo {
  @Annotation(Bar)
//            ^^^
// [diag.undefinedIdentifier] Undefined name 'Bar'.
// [diag.constWithNonConstantArgument] Arguments of a constant creation must be constant expressions.
  set Bar(int value) {}
}
"#,
        );
    }

    #[test]
    fn enum_constant() {
        check(
            r#"
var a = 42;

enum E {
  v(a);
//  ^
// [diag.constWithNonConstantArgument] Arguments of a constant creation must be constant expressions.
  const E(_);
}
"#,
        );
    }

    #[test]
    fn enum_constant_constant_context() {
        check(
            r#"
enum E {
  v([]);
  const E(_);
}
"#,
        );
    }

    #[test]
    fn instance_creation() {
        check(
            r#"
class A {
  const A(a);
}
f(p) { return const A(p); }
//                    ^
// [diag.constWithNonConstantArgument] Arguments of a constant creation must be constant expressions.
"#,
        );
    }

    #[test]
    fn issue47603() {
        check(
            r#"
class C {
  final void Function() c;
  const C(this.c);
}

void main() {
  const C(() {});
//        ^^^^^
// [diag.constWithNonConstantArgument] Arguments of a constant creation must be constant expressions.
}
"#,
        );
    }
}

/// Port of `pkg/analyzer/test/src/diagnostics/const_spread_expected_list_or_set_test.dart`.
mod const_spread_expected_list_or_set {
    use super::check;

    #[test]
    fn const_list_int() {
        check(
            r#"
const dynamic a = 5;
var b = const <int>[...a];
//                     ^
// [diag.constSpreadExpectedListOrSet] A list or a set is expected in this spread.
"#,
        );
    }

    #[test]
    fn const_list_int_const_variable() {
        check(
            r#"
const dynamic a = 5;
const x = <int>[...a];
//                 ^
// [diag.constSpreadExpectedListOrSet] A list or a set is expected in this spread.
"#,
        );
    }

    #[test]
    fn const_list_list() {
        check(
            r#"
const dynamic a = [5];
var b = const <int>[...a];
"#,
        );
    }

    #[test]
    fn const_list_map() {
        check(
            r#"
const dynamic a = <int, int>{0: 1};
var b = const <int>[...a];
//                     ^
// [diag.constSpreadExpectedListOrSet] A list or a set is expected in this spread.
"#,
        );
    }

    #[test]
    fn const_list_null() {
        check(
            r#"
const dynamic a = null;
var b = const <int>[...a];
//                     ^
// [diag.constSpreadExpectedListOrSet] A list or a set is expected in this spread.
"#,
        );
    }

    #[test]
    fn const_list_null_nullable() {
        check(
            r#"
const dynamic a = null;
var b = const <int>[...?a];
"#,
        );
    }

    #[test]
    fn const_list_set() {
        check(
            r#"
const dynamic a = <int>{5};
var b = const <int>[...a];
"#,
        );
    }

    #[test]
    fn const_set_int() {
        check(
            r#"
const dynamic a = 5;
var b = const <int>{...a};
//                     ^
// [diag.constSpreadExpectedListOrSet] A list or a set is expected in this spread.
"#,
        );
    }

    #[test]
    fn const_set_list() {
        check(
            r#"
const dynamic a = <int>[5];
var b = const <int>{...a};
"#,
        );
    }

    #[test]
    fn const_set_map() {
        check(
            r#"
const dynamic a = <int, int>{1: 2};
var b = const <int>{...a};
//                     ^
// [diag.constSpreadExpectedListOrSet] A list or a set is expected in this spread.
"#,
        );
    }

    #[test]
    fn const_set_null() {
        check(
            r#"
const dynamic a = null;
var b = const <int>{...a};
//                     ^
// [diag.constSpreadExpectedListOrSet] A list or a set is expected in this spread.
"#,
        );
    }

    #[test]
    fn const_set_null_nullable() {
        check(
            r#"
const dynamic a = null;
var b = const <int>{...?a};
"#,
        );
    }

    #[test]
    fn const_set_set() {
        check(
            r#"
const dynamic a = <int>{5};
var b = const <int>{...a};
"#,
        );
    }

    #[test]
    fn non_const_list_int() {
        check(
            r#"
const dynamic a = 5;
var b = <int>[...a];
"#,
        );
    }

    #[test]
    fn non_const_set_int() {
        check(
            r#"
const dynamic a = 5;
var b = <int>{...a};
"#,
        );
    }
}

/// Port of `pkg/analyzer/test/src/diagnostics/const_spread_expected_map_test.dart`.
mod const_spread_expected_map {
    use super::check;

    #[test]
    fn const_map_int() {
        check(
            r#"
const dynamic a = 5;
var b = const <int, int>{...a};
//                          ^
// [diag.constSpreadExpectedMap] A map is expected in this spread.
"#,
        );
    }

    #[test]
    fn const_map_list() {
        check(
            r#"
const dynamic a = <int>[5];
var b = const <int, int>{...a};
//                          ^
// [diag.constSpreadExpectedMap] A map is expected in this spread.
"#,
        );
    }

    #[test]
    fn const_map_map() {
        check(
            r#"
const dynamic a = <int, int>{1: 2};
var b = <int, int>{...a};
"#,
        );
    }

    #[test]
    fn const_map_null() {
        check(
            r#"
const dynamic a = null;
var b = const <int, int>{...a};
//                          ^
// [diag.constSpreadExpectedMap] A map is expected in this spread.
"#,
        );
    }

    #[test]
    fn const_map_null_nullable() {
        check(
            r#"
const dynamic a = null;
var b = <int, int>{...?a};
"#,
        );
    }

    #[test]
    fn const_map_set() {
        check(
            r#"
const dynamic a = <int>{5};
var b = const <int, int>{...a};
//                          ^
// [diag.constSpreadExpectedMap] A map is expected in this spread.
"#,
        );
    }

    #[test]
    fn non_const_map_int() {
        check(
            r#"
const dynamic a = 5;
var b = <int, int>{...a};
"#,
        );
    }

    #[test]
    fn non_const_map_map() {
        check(
            r#"
const dynamic a = {1: 2};
var b = <int, int>{...a};
"#,
        );
    }
}

/// Port of `pkg/analyzer/test/src/diagnostics/recursive_compile_time_constant_test.dart`.
mod recursive_compile_time_constant {
    use super::check;

    #[test]
    fn cycle() {
        check(
            r#"
const x = y + 1;
//    ^
// [diag.recursiveCompileTimeConstant] The compile-time constant expression depends on itself.
const y = x + 1;
//    ^
// [diag.recursiveCompileTimeConstant] The compile-time constant expression depends on itself.
"#,
        );
    }

    #[test]
    fn enum_constant_values() {
        check(
            r#"
enum E {
  v(values);
//^
// [diag.recursiveCompileTimeConstant] The compile-time constant expression depends on itself.
  const E(Object a);
}
"#,
        );
    }

    #[test]
    fn enum_constants() {
        check(
            r#"
enum E {
  v1(v2), v2(v1);
//^^
// [diag.recursiveCompileTimeConstant] The compile-time constant expression depends on itself.
//        ^^
// [diag.recursiveCompileTimeConstant] The compile-time constant expression depends on itself.
  const E(E other);
}
"#,
        );
    }

    #[test]
    fn enum_fields() {
        check(
            r#"
enum E {
  v;
  static const x = y + 1;
//             ^
// [diag.recursiveCompileTimeConstant] The compile-time constant expression depends on itself.
  static const y = x + 1;
//             ^
// [diag.recursiveCompileTimeConstant] The compile-time constant expression depends on itself.
}
"#,
        );
    }

    #[test]
    fn single_variable() {
        check(
            r#"
const x = x;
//    ^
// [diag.recursiveCompileTimeConstant] The compile-time constant expression depends on itself.
"#,
        );
    }

    #[test]
    fn single_variable_from_const_list() {
        check(
            r#"
const elems = const [
//    ^^^^^
// [diag.recursiveCompileTimeConstant] The compile-time constant expression depends on itself.
  const [
    1, elems, 3,
  ],
];
"#,
        );
    }
}

/// Port of `pkg/analyzer/test/src/diagnostics/recursive_constant_constructor_test.dart`.
mod recursive_constant_constructor {
    use super::check;

    #[test]
    fn new_head_named_redirecting_constructor_invocation() {
        check(
            r#"
class A {
  const new named() : this.named();
//      ^^^^^^^^^
// [diag.recursiveConstantConstructor] The constant constructor depends on itself.
//                    ^^^^^^^^^^^^
// [diag.recursiveConstructorRedirect] Constructors can't redirect to themselves either directly or indirectly.
}
"#,
        );
    }

    #[test]
    fn new_head_unnamed_redirecting_constructor_invocation() {
        check(
            r#"
class A {
  const new () : this();
//      ^^^
// [diag.recursiveConstantConstructor] The constant constructor depends on itself.
//               ^^^^^^
// [diag.recursiveConstructorRedirect] Constructors can't redirect to themselves either directly or indirectly.
}
"#,
        );
    }

    #[test]
    fn type_name_field() {
        check(
            r#"
class A {
  const A();
//      ^
// [diag.recursiveConstantConstructor] The constant constructor depends on itself.
  final m = const A();
}
"#,
        );
    }

    #[test]
    fn type_name_initializer_after_toplevel_var() {
        check(
            r#"
const y = const C();
//    ^
// [diag.recursiveCompileTimeConstant] The compile-time constant expression depends on itself.
class C {
  const C() : x = y;
//      ^
// [diag.recursiveConstantConstructor] The constant constructor depends on itself.
  final x;
}
"#,
        );
    }

    #[test]
    fn type_name_initializer_field() {
        check(
            r#"
class A {
  final A a;
  const A() : a = const A();
//      ^
// [diag.recursiveConstantConstructor] The constant constructor depends on itself.
}
"#,
        );
    }

    #[test]
    fn type_name_initializer_field_multiple_classes() {
        check(
            r#"
class B {
  final A a;
  const B() : a = const A();
//      ^
// [diag.recursiveConstantConstructor] The constant constructor depends on itself.
}
class A {
  final B b;
  const A() : b = const B();
//      ^
// [diag.recursiveConstantConstructor] The constant constructor depends on itself.
}
"#,
        );
    }
}

/// Port of `pkg/analyzer/test/src/diagnostics/const_eval_for_element_test.dart`.
mod const_eval_for_element {
    use super::check;

    #[test]
    #[ignore = "open: Dart reports const_initialized_with_non_constant_value at the literal (evaluation of the linked initializer)"]
    fn list_literal() {
        check(
            r#"
const x = [for (int i = 0; i < 3; i++) i];
//        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
//         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.constEvalForElement] Constant expressions don't support 'for' elements.
"#,
        );
    }

    #[test]
    #[ignore = "open: Dart reports const_initialized_with_non_constant_value at the literal (evaluation of the linked initializer)"]
    fn list_literal_for_in() {
        check(
            r#"
const Set set = {};
const x = [for(final i in set) i];
//        ^^^^^^^^^^^^^^^^^^^^^^^
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
//         ^^^^^^^^^^^^^^^^^^^^^
// [diag.constEvalForElement] Constant expressions don't support 'for' elements.
"#,
        );
    }

    #[test]
    #[ignore = "open: Dart reports const_initialized_with_non_constant_value at the literal (evaluation of the linked initializer)"]
    fn map_literal_for_in() {
        check(
            r#"
const x = {for (final i in const []) i: null};
//        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
//         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.constEvalForElement] Constant expressions don't support 'for' elements.
"#,
        );
    }

    #[test]
    #[ignore = "open: Dart reports const_initialized_with_non_constant_value at the literal (evaluation of the linked initializer)"]
    fn map_literal_for_in_nested() {
        check(
            r#"
const x = {if (true) for (final i in const []) i: null};
//        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
//                   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.constEvalForElement] Constant expressions don't support 'for' elements.
"#,
        );
    }

    #[test]
    #[ignore = "open: Dart reports const_initialized_with_non_constant_value at the literal (evaluation of the linked initializer)"]
    fn set_literal_for_in() {
        check(
            r#"
const Set set = {};
const x = {for (final i in set) i};
//        ^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
//         ^^^^^^^^^^^^^^^^^^^^^^
// [diag.constEvalForElement] Constant expressions don't support 'for' elements.
"#,
        );
    }
}

/// Port of `pkg/analyzer/test/src/diagnostics/list_element_type_not_assignable_test.dart`.
mod list_element_type_not_assignable {
    use super::check;

    #[test]
    fn const_int_int() {
        check(
            r#"
var v1 = <int> [42];
var v2 = const <int> [42];
"#,
        );
    }
}

/// Port of `pkg/analyzer/test/src/diagnostics/set_element_type_not_assignable_test.dart`.
mod set_element_type_not_assignable {
    use super::check;

    #[test]
    fn const_if_element_then_else_false_int_int() {
        check(
            r#"
const dynamic a = 0;
const dynamic b = 0;
var v = const <int>{if (1 < 0) a else b};
"#,
        );
    }

    #[test]
    fn const_if_element_then_else_false_int_string() {
        check(
            r#"
const dynamic a = 0;
const dynamic b = 'b';
var v = const <int>{if (1 < 0) a else b};
//                                    ^
// [diag.setElementTypeNotAssignable] The element type 'String' can't be assigned to the set type 'int'.
"#,
        );
    }

    #[test]
    #[ignore = "reported by the literal element verifier (unit D11)"]
    fn const_if_element_then_false_int_string() {
        check(
            r#"
var v = const <int>{if (1 < 0) 'a'};
//                             ^^^
// [diag.setElementTypeNotAssignable] The element type 'String' can't be assigned to the set type 'int'.
"#,
        );
    }

    #[test]
    fn const_if_element_then_false_int_string_dynamic() {
        check(
            r#"
const dynamic a = 'a';
var v = const <int>{if (1 < 0) a};
"#,
        );
    }

    #[test]
    fn const_if_element_then_true_int_int() {
        check(
            r#"
const dynamic a = 0;
var v = const <int>{if (true) a};
"#,
        );
    }

    #[test]
    fn const_if_element_then_true_int_string() {
        check(
            r#"
const dynamic a = 'a';
var v = const <int>{if (true) a};
//                            ^
// [diag.setElementTypeNotAssignable] The element type 'String' can't be assigned to the set type 'int'.
"#,
        );
    }

    #[test]
    fn const_int_int_dynamic() {
        check(
            r#"
const dynamic a = 42;
var v = const <int>{a};
"#,
        );
    }

    #[test]
    fn const_int_int_value() {
        check(
            r#"
var v = const <int>{42};
"#,
        );
    }

    #[test]
    fn const_int_null_dynamic() {
        check(
            r#"
const a = null;
var v = const <int>{a};
//                  ^
// [diag.setElementTypeNotAssignableNullability] The element type 'Null' can't be assigned to the set type 'int'.
"#,
        );
    }

    #[test]
    fn const_int_null_value() {
        check(
            r#"
var v = const <int>{null};
//                  ^^^^
// [diag.setElementTypeNotAssignableNullability] The element type 'Null' can't be assigned to the set type 'int'.
"#,
        );
    }

    #[test]
    fn const_int_string_dynamic() {
        check(
            r#"
const dynamic x = 'abc';
var v = const <int>{x};
//                  ^
// [diag.setElementTypeNotAssignable] The element type 'String' can't be assigned to the set type 'int'.
"#,
        );
    }

    #[test]
    fn const_int_string_value() {
        check(
            r#"
var v = const <int>{'abc'};
//                  ^^^^^
// [diag.setElementTypeNotAssignable] The element type 'String' can't be assigned to the set type 'int'.
"#,
        );
    }

    #[test]
    fn const_spread_int_int() {
        check(
            r#"
var v = const <int>{...[0, 1]};
"#,
        );
    }

    #[test]
    fn const_string_question_null_dynamic() {
        check(
            r#"
const a = null;
var v = const <String?>{a};
"#,
        );
    }

    #[test]
    fn const_string_question_null_value() {
        check(
            r#"
var v = const <String?>{null};
"#,
        );
    }

    #[test]
    fn non_const_if_element_then_else_false_int_dynamic() {
        check(
            r#"
const dynamic a = 'a';
const dynamic b = 'b';
var v = <int>{if (1 < 0) a else b};
"#,
        );
    }

    #[test]
    fn non_const_if_element_then_else_false_int_int() {
        check(
            r#"
const dynamic a = 0;
const dynamic b = 0;
var v = <int>{if (1 < 0) a else b};
"#,
        );
    }

    #[test]
    #[ignore = "reported by the literal element verifier (unit D11)"]
    fn non_const_if_element_then_false_int_string() {
        check(
            r#"
var v = <int>[if (1 < 0) 'a'];
//                       ^^^
// [diag.listElementTypeNotAssignable] The element type 'String' can't be assigned to the list type 'int'.
"#,
        );
    }

    #[test]
    fn non_const_if_element_then_true_int_dynamic() {
        check(
            r#"
const dynamic a = 'a';
var v = <int>{if (true) a};
"#,
        );
    }

    #[test]
    fn non_const_if_element_then_true_int_int() {
        check(
            r#"
const dynamic a = 0;
var v = <int>{if (true) a};
"#,
        );
    }

    #[test]
    fn non_const_spread_int_int() {
        check(
            r#"
var v = <int>{...[0, 1]};
"#,
        );
    }

    #[test]
    fn not_const_int_string_dynamic() {
        check(
            r#"
const dynamic x = 'abc';
var v = <int>{x};
"#,
        );
    }

    #[test]
    #[ignore = "reported by the literal element verifier (unit D11)"]
    fn not_const_int_string_value() {
        check(
            r#"
var v = <int>{'abc'};
//            ^^^^^
// [diag.setElementTypeNotAssignable] The element type 'String' can't be assigned to the set type 'int'.
"#,
        );
    }
}

/// Port of `pkg/analyzer/test/src/diagnostics/constant_pattern_never_matches_value_type_test.dart`.
mod constant_pattern_never_matches_value_type {
    use super::check;

    #[test]
    fn bool_bool() {
        check(
            r#"
void f(bool x) {
  if (x case (true)) {}
}
"#,
        );
    }

    #[test]
    fn bool_int() {
        check(
            r#"
void f(int x) {
  if (x case (true)) {}
//            ^^^^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'int' can never be equal to this constant of type 'bool'.
}
"#,
        );
    }

    #[test]
    fn bool_list_of_bool() {
        check(
            r#"
void f(List<bool> x) {
  if (x case (true)) {}
//            ^^^^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'List<bool>' can never be equal to this constant of type 'bool'.
}
"#,
        );
    }

    #[test]
    fn bool_type_parameter_bound_bool() {
        check(
            r#"
void f<T extends bool>(T x) {
  if (x case (true)) {}
}
"#,
        );
    }

    #[test]
    fn bool_type_parameter_bound_bool_nested() {
        check(
            r#"
void f<T extends bool>(List<T> x) {
  if (x case [true]) {}
}
"#,
        );
    }

    #[test]
    fn bool_type_parameter_bound_num() {
        check(
            r#"
void f<T extends num>(T x) {
  if (x case (true)) {}
//            ^^^^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'T' can never be equal to this constant of type 'bool'.
}
"#,
        );
    }

    #[test]
    fn bool_type_parameter_bound_num_nested() {
        check(
            r#"
void f<T extends num>(List<T> x) {
  if (x case [true]) {}
//            ^^^^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'T' can never be equal to this constant of type 'bool'.
}
"#,
        );
    }

    #[test]
    fn custom_not_primitive_equality_constant_is_subtype_of_value() {
        check(
            r#"
void f(A x) {
  if (x case const B()) {}
}

class A {
  const A();
}

class B extends A {
  const B();
  bool operator ==(other) => true;
}
"#,
        );
    }

    #[test]
    fn custom_not_primitive_equality_constant_is_supertype_of_value() {
        check(
            r#"
void f(B x) {
  if (x case const A()) {}
}

class A {
  const A();
  bool operator ==(other) => true;
}

class B extends A {
  const B();
}
"#,
        );
    }

    #[test]
    fn custom_primitive_equality_constant_is_same_type_as_value() {
        check(
            r#"
void f(A x) {
  if (x case const A()) {}
}

class A {
  const A();
}
"#,
        );
    }

    #[test]
    fn custom_primitive_equality_constant_is_subtype_of_value() {
        check(
            r#"
void f(A x) {
  if (x case const B()) {}
}

class A {
  const A();
}

class B extends A {
  const B();
}
"#,
        );
    }

    #[test]
    fn custom_primitive_equality_constant_is_supertype_of_value() {
        check(
            r#"
void f(B x) {
  if (x case const A()) {}
//           ^^^^^^^^^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'B' can never be equal to this constant of type 'A'.
}

class A {
  const A();
}

class B extends A {
  const B();
}
"#,
        );
    }

    #[test]
    fn custom_primitive_equality_generic_different_element_constant_is_subtype_of_value() {
        check(
            r#"
void f(A<int> x) {
  if (x case const B()) {}
}

class A<T> {
  const A();
}

class B extends A<int> {
  const B();
}
"#,
        );
    }

    #[test]
    fn custom_primitive_equality_generic_different_element_constant_is_supertype_of_value() {
        check(
            r#"
void f(B x) {
  if (x case const A<int>()) {}
//           ^^^^^^^^^^^^^^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'B' can never be equal to this constant of type 'A<int>'.
}

class A<T> {
  const A();
}

class B extends A<int> {
  const B();
}
"#,
        );
    }

    #[test]
    fn custom_primitive_equality_generic_same_element_constant_is_same_type_as_value() {
        check(
            r#"
void f(A<int> x) {
  if (x case const A<int>()) {}
}

class A<T> {
  const A();
}
"#,
        );
    }

    #[test]
    fn custom_primitive_equality_generic_same_element_constant_is_subtype_of_value() {
        check(
            r#"
void f(A<num> x) {
  if (x case const A<int>()) {}
}

class A<T> {
  const A();
}
"#,
        );
    }

    #[test]
    fn custom_primitive_equality_generic_same_element_constant_is_supertype_of_value() {
        check(
            r#"
void f(A<int> x) {
  if (x case const A<num>()) {}
//           ^^^^^^^^^^^^^^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'A<int>' can never be equal to this constant of type 'A<num>'.
}

class A<T> {
  const A();
}
"#,
        );
    }

    #[test]
    fn custom_primitive_equality_generic_same_element_type_parameter() {
        check(
            r#"
void f<T>(A<T> x) {
  if (x case const A<int>()) {}
}

class A<T> {
  const A();
}
"#,
        );
    }

    #[test]
    fn custom_primitive_equality_generic_same_element_type_parameter_contravariant() {
        check(
            r#"
void f<T>(A<void Function(T)> x) {
  if (x case const A<void Function(int)>()) {}
}

class A<T> {
  const A();
}
"#,
        );
    }

    #[test]
    fn int_bool() {
        check(
            r#"
void f(bool x) {
  if (x case (0)) {}
//            ^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'bool' can never be equal to this constant of type 'int'.
}
"#,
        );
    }

    #[test]
    fn int_double() {
        check(
            r#"
void f(double x) {
  if (x case (zero)) {}
}

const zero = 0;
"#,
        );
    }

    #[test]
    fn int_extension_type_bool() {
        check(
            r#"
extension type E(bool it) {}

void f(E x) {
  if (x case (0)) {}
//            ^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'bool' can never be equal to this constant of type 'int'.
}
"#,
        );
    }

    #[test]
    fn int_extension_type_int() {
        check(
            r#"
extension type E(int it) {}

void f(E x) {
  if (x case (0)) {}
}
"#,
        );
    }

    #[test]
    fn int_function_type() {
        check(
            r#"
void f(void Function() x) {
  if (x case (0)) {}
//            ^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'void Function()' can never be equal to this constant of type 'int'.
}

class A {}
"#,
        );
    }

    #[test]
    fn int_int() {
        check(
            r#"
void f(int x) {
  if (x case (0)) {}
}
"#,
        );
    }

    #[test]
    fn int_int_question() {
        check(
            r#"
void f(int? x) {
  if (x case (0)) {}
}
"#,
        );
    }

    #[test]
    fn int_num() {
        check(
            r#"
void f(num x) {
  if (x case (0)) {}
}
"#,
        );
    }

    #[test]
    fn int_other_class() {
        check(
            r#"
void f(A x) {
  if (x case (0)) {}
//            ^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'A' can never be equal to this constant of type 'int'.
}

class A {}
"#,
        );
    }

    #[test]
    fn int_record_type() {
        check(
            r#"
void f((int, int) x) {
  if (x case 0) {}
//           ^
// [diag.constantPatternNeverMatchesValueType] The matched value type '(int, int)' can never be equal to this constant of type 'int'.
}

class A {}
"#,
        );
    }

    #[test]
    fn int_string() {
        check(
            r#"
void f(String x) {
  if (x case (0)) {}
//            ^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'String' can never be equal to this constant of type 'int'.
}
"#,
        );
    }

    #[test]
    fn null_function_type() {
        check(
            r#"
void f(void Function() x) {
  if (x case null) {}
//           ^^^^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'void Function()' can never be equal to this constant of type 'Null'.
//                 ^^
// [diag.deadCode] Dead code.
}
"#,
        );
    }

    #[test]
    fn null_function_type_question() {
        check(
            r#"
void f(void Function()? x) {
  if (x case null) {}
}
"#,
        );
    }

    #[test]
    fn null_int() {
        check(
            r#"
void f(int x) {
  if (x case null) {}
//           ^^^^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'int' can never be equal to this constant of type 'Null'.
//                 ^^
// [diag.deadCode] Dead code.
}
"#,
        );
    }

    #[test]
    fn null_int_question() {
        check(
            r#"
void f(int? x) {
  if (x case null) {}
}
"#,
        );
    }

    #[test]
    fn null_record_type() {
        check(
            r#"
void f((int, int) x) {
  if (x case null) {}
//           ^^^^
// [diag.constantPatternNeverMatchesValueType] The matched value type '(int, int)' can never be equal to this constant of type 'Null'.
//                 ^^
// [diag.deadCode] Dead code.
}
"#,
        );
    }

    #[test]
    fn null_record_type_question() {
        check(
            r#"
void f((int, int)? x) {
  if (x case null) {}
}
"#,
        );
    }

    #[test]
    fn null_type_parameter_type_not_nullable_bound() {
        check(
            r#"
void f<T extends Object>(T x) {
  if (x case null) {}
//           ^^^^
// [diag.constantPatternNeverMatchesValueType] The matched value type 'T' can never be equal to this constant of type 'Null'.
//                 ^^
// [diag.deadCode] Dead code.
}
"#,
        );
    }

    #[test]
    fn null_type_parameter_type_not_nullable_bound_question() {
        check(
            r#"
void f<T extends Object>(T? x) {
  if (x case null) {}
}
"#,
        );
    }

    #[test]
    fn null_type_parameter_type_nullable_bound() {
        check(
            r#"
void f<T>(T x) {
  if (x case null) {}
}
"#,
        );
    }

    #[test]
    fn null_type_parameter_type_nullable_bound_question() {
        check(
            r#"
void f<T>(T? x) {
  if (x case null) {}
}
"#,
        );
    }
}

/// Port of `pkg/analyzer/test/src/diagnostics/const_with_type_parameters_test.dart`.
mod const_with_type_parameters {
    use super::check;

    #[test]
    fn as_expression_function_type() {
        check(
            r#"
void f<T>(T a) {}
void g() {
  const [f as void Function<T>(T, [int])];
//       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.listElementTypeNotAssignable] The element type 'void Function<T>(T)' can't be assigned to the list type 'void Function<T>(T, [int])'.
}
"#,
        );
    }

    #[test]
    fn default_value() {
        check(
            r#"
class A<T> {
  void m([fn = A<T>.new]) {}
//               ^
// [diag.constWithTypeParametersConstructorTearoff] A constant constructor tearoff can't use a type parameter as a type argument.
}
"#,
        );
    }

    #[test]
    fn default_value_field_formal_parameter() {
        check(
            r#"
class A<T> {
  A<T> Function() fn;
  A([this.fn = A<T>.new]);
//               ^
// [diag.constWithTypeParametersConstructorTearoff] A constant constructor tearoff can't use a type parameter as a type argument.
}
"#,
        );
    }

    #[test]
    fn default_value_no_type_variable_inferred_from_parameter() {
        check(
            r#"
class A<T> {
  void m([A<T> Function() fn = A.new]) {}
//                             ^^^^^
// [diag.invalidAssignment] A value of type 'A<dynamic> Function()' can't be assigned to a variable of type 'A<T> Function()'.
}
"#,
        );
    }

    #[test]
    fn field_value_const_class() {
        check(
            r#"
class A<T> {
  const A();
  final x = A<T>.new;
//            ^
// [diag.constWithTypeParametersConstructorTearoff] A constant constructor tearoff can't use a type parameter as a type argument.
}
"#,
        );
    }

    #[test]
    fn applied_type_parameter_default_constructor_value() {
        check(
            r#"
void f<T>(T t) => t;

class C<T> {
  final void Function(T) p;
  const C({this.p = f});
//                  ^
// [diag.constWithTypeParametersFunctionTearoff] A constant function tearoff can't use a type parameter as a type argument.
}
"#,
        );
    }

    #[test]
    fn applied_type_parameter_default_function_value() {
        check(
            r#"
void f<T>(T t) => t;

void bar<T>([void Function(T) p = f]) {}
//                                ^
// [diag.constWithTypeParametersFunctionTearoff] A constant function tearoff can't use a type parameter as a type argument.
"#,
        );
    }

    #[test]
    fn applied_type_parameter_default_method_value() {
        check(
            r#"
void f<T>(T t) => t;

class C<T> {
  void foo([void Function(T) p = f]) {}
//                               ^
// [diag.constWithTypeParametersFunctionTearoff] A constant function tearoff can't use a type parameter as a type argument.
}
"#,
        );
    }

    #[test]
    fn applied_type_parameter_nested() {
        check(
            r#"
void f<T>(T t) => t;

void bar<T>([void Function(List<T>) p = f]) {}
//                                      ^
// [diag.constWithTypeParametersFunctionTearoff] A constant function tearoff can't use a type parameter as a type argument.
"#,
        );
    }

    #[test]
    fn applied_type_parameter_nested_function() {
        check(
            r#"
void f<T>(T t) => t;

void bar<T>([void Function(T Function()) p = f]) {}
//                                           ^
// [diag.constWithTypeParametersFunctionTearoff] A constant function tearoff can't use a type parameter as a type argument.
"#,
        );
    }

    #[test]
    fn default_value_2() {
        check(
            r#"
void f<T>(T a) {}
class A<U> {
  void m([void Function(U) fn = f<U>]) {}
//                                ^
// [diag.constWithTypeParametersFunctionTearoff] A constant function tearoff can't use a type parameter as a type argument.
}
"#,
        );
    }

    #[test]
    fn field_value_const_class_2() {
        check(
            r#"
void f<T>(T a) {}
class A<U> {
  const A();
//^^^^^
// [diag.constConstructorWithFieldInitializedByNonConst] Can't define the 'const' constructor because the field 'x' is initialized with a non-constant value.
  final x = f<U>;
//            ^
// [diag.constWithTypeParametersFunctionTearoff] A constant function tearoff can't use a type parameter as a type argument.
}
"#,
        );
    }

    #[test]
    fn field_value_extension() {
        check(
            r#"
void f<T>(T a) {}
class A<U> {}
extension<U> on A<U> {
  final x = f<U>;
//      ^
// [diag.extensionDeclaresInstanceField] Extensions can't declare instance fields.
}
"#,
        );
    }

    #[test]
    fn field_value_non_const_class() {
        check(
            r#"
void f<T>(T a) {}
class A<U> {
  final x = f<U>;
}
"#,
        );
    }
}

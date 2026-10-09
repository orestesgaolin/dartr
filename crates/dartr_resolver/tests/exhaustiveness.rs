//! Port of the analyzer tests of the switch exhaustiveness diagnostics:
//! `pkg/analyzer/test/src/diagnostics/non_exhaustive_switch_test.dart`,
//! `unreachable_switch_case_test.dart` and
//! `unreachable_switch_default_test.dart`.
//!
//! Each test resolves the Dart code through `Driver::analyze_library`,
//! builds a `ConstantEvaluationEngine` over the resolved units, fills the
//! constant pattern values and the map pattern key values of each switch
//! like the Dart `ConstantVerifier.visitConstantPattern` /
//! `visitMapPattern` do, and calls `validate_switch_exhaustiveness`
//! directly (the constant verifier that calls it is in another branch).
//!
//! The expectations are the inline comments of the Dart tests (`^^^` marks
//! the range in the code line above, `// [diag.code] message` the
//! diagnostic). Only the codes that `validate_switch_exhaustiveness`
//! reports are checked: code, offset, length and message.

mod support;

use dartr_ast::{
    ConstantPattern, MapPatternEntry, NodeId, NodeKind, SwitchExpression, SwitchStatement,
};
use dartr_constant::{Constant, DartObjectImpl, DeclaredVariables};
use dartr_element::{TypeId, TypeKind};
use dartr_resolver::constant::evaluation::{ConstantEvaluationEngine, ConstantVisitor, NodeRef};
use dartr_resolver::constant::exhaustiveness::{
    ExhaustivenessCache, SwitchExhaustivenessInput, validate_switch_exhaustiveness,
};
use dartr_resolver::library_analyzer::ExternalUnitCache;
use dartr_resolver::options::AnalysisOptions;
use dartr_typesystem::TypeSystem;
use indexmap::IndexMap;
use support::{analyze, is_attached};

/// The codes that `validate_switch_exhaustiveness` reports.
const CODES: &[&str] = &[
    "nonExhaustiveSwitchExpression",
    "nonExhaustiveSwitchExpressionPrivate",
    "nonExhaustiveSwitchStatement",
    "nonExhaustiveSwitchStatementPrivate",
    "unreachableSwitchCase",
    "unreachableSwitchDefault",
];

/// A diagnostic as (camel case code, offset, length, message).
type Expected = (String, usize, usize, String);

/// Parses the inline expectations of a Dart test: a comment line with only
/// `^` marks the range in the last code line, a `// [diag.code] message`
/// line adds a diagnostic at the last range.
fn expectations(source: &str) -> Vec<Expected> {
    let mut result = vec![];
    let mut offset = 0;
    let mut code_line_start = 0;
    let mut range = (0, 0);
    for line in source.split_inclusive('\n') {
        let trimmed = line.trim();
        let comment = trimmed.strip_prefix("//").map(str::trim);
        match comment {
            Some(c) if !c.is_empty() && c.chars().all(|ch| ch == '^') => {
                let column = line.find('^').unwrap();
                range = (code_line_start + column, c.len());
            }
            Some(c) if c.starts_with("[diag.") => {
                let end = c.find(']').unwrap();
                let code = &c[6..end];
                let message = c[end + 1..].trim();
                if CODES.contains(&code) {
                    result.push((code.to_string(), range.0, range.1, message.to_string()));
                }
            }
            _ => code_line_start = offset,
        }
        offset += line.len();
    }
    result.sort();
    result
}

/// Runs the switch exhaustiveness check on every switch of `main.dart`
/// ([main]; [others] are more files of the package) and compares the
/// diagnostics with the expectations of [main].
fn check(main: &str, others: &[(&str, &str)]) {
    let mut files = vec![("main.dart", main)];
    files.extend_from_slice(others);
    let Some(analyzed) = analyze(&files) else {
        eprintln!("no Dart SDK found, skipping");
        return;
    };
    let world = &analyzed.driver.state.world;
    let tp = &*analyzed.tp;
    let library = analyzed.library.library;
    let units = &analyzed.library.units;
    for unit in units {
        assert!(
            unit.panic.is_none(),
            "resolution panicked: {:?}",
            unit.panic
        );
    }
    let declared_variables = DeclaredVariables::new();
    let external = ExternalUnitCache::new(
        world,
        tp,
        AnalysisOptions::default(),
        analyzed.driver.unit_sources(),
    );
    let engine = ConstantEvaluationEngine::new(
        world,
        tp,
        library,
        &declared_variables,
        units,
        Some(&external),
    );
    *engine.values.borrow_mut() = analyzed.library.constants.clone();

    let unit = &units[0];
    let ast = &unit.ast;
    let ctx = engine.ctx(unit);
    let ts = TypeSystem::new(ctx);
    let root = unit.unit.raw();
    let static_type = |n: NodeId| {
        unit.tables
            .static_type
            .get(n)
            .copied()
            .unwrap_or(TypeId::INVALID)
    };

    // The switches in source order, and for each node its enclosing switch
    // (Dart `_withConstantPatternValues` gives each switch its own maps).
    let nodes: Vec<NodeId> = (0..ast.node_count())
        .map(NodeId::from_index)
        .filter(|&n| is_attached(ast, root, n))
        .collect();
    let enclosing_switch = |mut n: NodeId| loop {
        n = ast.parent(n)?;
        if matches!(
            ast.kind(n),
            NodeKind::SwitchStatement | NodeKind::SwitchExpression
        ) {
            return Some(n);
        }
    };
    let evaluate = |expression: NodeId| -> Option<DartObjectImpl> {
        let visitor = ConstantVisitor::new(&engine, library, None);
        match visitor.evaluate_constant(NodeRef::new(0, expression)) {
            Constant::Value(value) => Some(value),
            Constant::Invalid(_) => None,
        }
    };
    let mut switches: Vec<NodeId> = nodes
        .iter()
        .copied()
        .filter(|&n| {
            matches!(
                ast.kind(n),
                NodeKind::SwitchStatement | NodeKind::SwitchExpression
            )
        })
        .collect();
    switches.sort_by_key(|&n| ast.offset(n));

    let mut cache = ExhaustivenessCache::default();
    let mut diagnostics = vec![];
    for switch in switches {
        let mut constant_pattern_values = IndexMap::new();
        let mut map_pattern_key_values = IndexMap::new();
        for &n in &nodes {
            if enclosing_switch(n) != Some(switch) {
                continue;
            }
            if let Some(p) = ast.cast::<ConstantPattern>(n) {
                // Dart `visitConstantPattern`.
                let expression = dartr_resolver::ast_ext::un_parenthesized(ast, ast[p].expression);
                if matches!(*ctx.ty(static_type(expression.raw())), TypeKind::Invalid) {
                    continue;
                }
                if let Some(value) = evaluate(expression.raw()) {
                    constant_pattern_values.insert(n, value);
                }
            } else if let Some(e) = ast.cast::<MapPatternEntry>(n) {
                // Dart `visitMapPattern`.
                let key = ast[e].key.raw();
                if let Some(value) = evaluate(key) {
                    map_pattern_key_values.insert(key, value);
                }
            }
        }
        let (is_switch_expression, must_be_exhaustive) =
            if ast.cast::<SwitchExpression>(switch).is_some() {
                (true, true)
            } else {
                let s = ast.cast::<SwitchStatement>(switch).unwrap();
                let scrutinee_type = static_type(ast[s].expression.raw());
                (false, ts.is_always_exhaustive(scrutinee_type))
            };
        let input = SwitchExhaustivenessInput {
            node: switch,
            map_pattern_key_values: &map_pattern_key_values,
            constant_pattern_values: &constant_pattern_values,
            must_be_exhaustive,
            is_switch_expression,
        };
        validate_switch_exhaustiveness(&engine, &mut cache, 0, &input, &mut diagnostics);
    }

    let mut actual: Vec<Expected> = diagnostics
        .iter()
        .map(|d| {
            (
                d.code.camel_case_name.to_string(),
                d.offset,
                d.length,
                d.message.clone(),
            )
        })
        .collect();
    actual.sort();
    assert_eq!(actual, expectations(main), "\n{main}");
}

// ---------------------------------------------------------------------------
// The tests
// ---------------------------------------------------------------------------

#[test]
fn expression_bool_true() {
    check(
        r#"
Object f(bool x) {
  return switch (x) {
//       ^^^^^^
// [diag.nonExhaustiveSwitchExpression] The type 'bool' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'false'.
    true => 0,
  };
}
"#,
        &[],
    );
}

#[test]
fn expression_bool_true_false() {
    check(
        r#"
Object f(bool x) {
  return switch (x) {
    true => 1,
    false => 0,
  };
}
"#,
        &[],
    );
}

#[test]
fn expression_class_int_wildcard() {
    check(
        r#"
Object f(int x) {
  return switch (x) {
    0 => 0,
    _ => 1,
  };
}
"#,
        &[],
    );
}

#[test]
fn expression_class_with_field_wildcard() {
    check(
        r#"
Object f(int x) {
  return switch (x) {
    int(isEven: true) => 0,
    _ => 1,
  };
}
"#,
        &[],
    );
}

#[test]
fn expression_enum_2at2_has_when() {
    check(
        r#"
enum E {
  a, b
}

Object f(E x) {
  return switch (x) {
//       ^^^^^^
// [diag.nonExhaustiveSwitchExpression] The type 'E' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'E.a'.
    E.a when 1 == 0 => 0,
    E.b => 1,
  };
}
"#,
        &[],
    );
}

#[test]
fn expression_invalid_type_empty() {
    check(
        r#"
void f(Unresolved x) => switch (x) {};
//     ^^^^^^^^^^
// [diag.undefinedClass] Undefined class 'Unresolved'.
"#,
        &[],
    );
}

#[test]
fn expression_private_enum() {
    check(
        r#"
import 'private_enum.dart';

Object f() {
  return switch (e()) {
//       ^^^^^^
// [diag.nonExhaustiveSwitchExpressionPrivate] The enum '_E' isn't exhaustively matched by the switch cases because some of the enum constants are private.
  };
}
"#,
        &[(
            "private_enum.dart",
            r#"
enum _E { a, b }
_E e() => _E.a;
"#,
        )],
    );
}

#[test]
fn expression_private_enum_same_library() {
    check(
        r#"
enum _E { a, b }
//        ^
// [diag.unusedField] The value of the field 'a' isn't used.
//           ^
// [diag.unusedField] The value of the field 'b' isn't used.

Object f(_E e) {
  return switch (e) {
//       ^^^^^^
// [diag.nonExhaustiveSwitchExpression] The type '_E' isn't exhaustively matched by the switch cases since it doesn't match the pattern '_E.a'.
  };
}
"#,
        &[],
    );
}

#[test]
fn expression_private_enum_constant() {
    check(
        r#"
import 'private_enum.dart';

Object f(E e) {
  return switch (e) {
//       ^^^^^^
// [diag.nonExhaustiveSwitchExpression] The type 'E' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'E.b'.
    E.a => 0,
  };
}
"#,
        &[(
            "private_enum.dart",
            r#"
enum E { a, b, _c }
"#,
        )],
    );
}

#[test]
fn expression_private_enum_constant_only() {
    check(
        r#"
import 'private_enum.dart';

Object f(E e) {
  return switch (e) {
//       ^^^^^^
// [diag.nonExhaustiveSwitchExpressionPrivate] The enum 'E' isn't exhaustively matched by the switch cases because some of the enum constants are private.
    E.a => 0,
    E.b => 1,
  };
}
"#,
        &[(
            "private_enum.dart",
            r#"
enum E { a, b, _c }
"#,
        )],
    );
}

#[test]
fn expression_private_enum_constant_same_library() {
    check(
        r#"
enum E { a, b, _c }
//             ^^
// [diag.unusedField] The value of the field '_c' isn't used.
Object f(E e) {
  return switch (e) {
//       ^^^^^^
// [diag.nonExhaustiveSwitchExpression] The type 'E' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'E._c'.
    E.a => 0,
    E.b => 1,
  };
}
"#,
        &[],
    );
}

#[test]
fn expression_private_sealed() {
    check(
        r#"
import 'private_sealed.dart';

Object f() {
  return switch (a()) {
//       ^^^^^^
// [diag.nonExhaustiveSwitchExpression] The type '_A' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'B()'.
  };
}
"#,
        &[(
            "private_sealed.dart",
            r#"
sealed class _A {}
class B extends _A {}
_A a() => B();
"#,
        )],
    );
}

#[test]
fn statement_always_exhaustive_bool_true() {
    check(
        r#"
void f(bool x) {
  switch (x) {
//^^^^^^
// [diag.nonExhaustiveSwitchStatement] The type 'bool' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'false'.
    case true:
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_bool_true_false() {
    check(
        r#"
void f(bool x) {
  switch (x) {
    case true:
    case false:
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_bool_wildcard_typed_bool() {
    check(
        r#"
void f(bool x) {
  switch (x) {
    case bool _:
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_bool_wildcard_typed_int() {
    check(
        r#"
void f(bool x) {
  switch (x) {
//^^^^^^
// [diag.nonExhaustiveSwitchStatement] The type 'bool' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'true'.
    case int _:
//       ^^^
// [diag.patternNeverMatchesValueType] The matched value type 'bool' can never match the required type 'int'.
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_bool_wildcard_untyped() {
    check(
        r#"
void f(bool x) {
  switch (x) {
    case _:
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_bool_nullable_true_false() {
    check(
        r#"
void f(bool? x) {
  switch (x) {
//^^^^^^
// [diag.nonExhaustiveSwitchStatement] The type 'bool?' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'null'.
    case true:
    case false:
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_bool_nullable_true_false_null() {
    check(
        r#"
void f(bool? x) {
  switch (x) {
    case true:
    case false:
    case null:
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_enum_2at1() {
    check(
        r#"
enum E {
  a, b
}

void f(E x) {
  switch (x) {
//^^^^^^
// [diag.nonExhaustiveSwitchStatement] The type 'E' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'E.b'.
    case E.a:
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_enum_2at2_cases() {
    check(
        r#"
enum E {
  a, b
}

void f(E x) {
  switch (x) {
    case E.a:
    case E.b:
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_enum_2at2_has_when() {
    check(
        r#"
enum E {
  a, b
}

void f(E x) {
  switch (x) {
//^^^^^^
// [diag.nonExhaustiveSwitchStatement] The type 'E' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'E.a'.
    case E.a when 1 == 0:
    case E.b:
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_enum_2at2_logical_or() {
    check(
        r#"
enum E {
  a, b
}

void f(E x) {
  switch (x) {
    case E.a || E.b:
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_enum_cannot_compute() {
    check(
        r#"
enum E {
  v1(v2), v2(v1);
//^^
// [diag.recursiveCompileTimeConstant] The compile-time constant expression depends on itself.
//        ^^
// [diag.recursiveCompileTimeConstant] The compile-time constant expression depends on itself.
  const E(Object f);
}

void f(E x) {
  switch (x) {
    case E.v1:
    case E.v2:
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_null_has_error() {
    check(
        r#"
void f(Null x) {
  switch (x) {}
//^^^^^^
// [diag.nonExhaustiveSwitchStatement] The type 'Null' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'null'.
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_null_no_error() {
    check(
        r#"
void f(Null x) {
  switch (x) {
    case null:
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_record_type_bool_bool_4at4() {
    check(
        r#"
void f((bool, bool) x) {
  switch (x) {
    case (false, false):
    case (false, true):
    case (true, false):
    case (true, true):
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_sealed_class_2at1() {
    check(
        r#"
sealed class A {}
class B extends A {}
class C extends A {}

void f(A x) {
  switch (x) {
//^^^^^^
// [diag.nonExhaustiveSwitchStatement] The type 'A' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'C()'.
    case B():
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_sealed_class_2at2() {
    check(
        r#"
sealed class A {}
class B extends A {}
class C extends A {}

void f(A x) {
  switch (x) {
    case B():
      break;
    case C():
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_sealed_class_2at2_wildcard() {
    check(
        r#"
sealed class A {}
class B extends A {}
class C extends A {}

void f(A x) {
  switch (x) {
    case B():
      break;
    case _:
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_sealed_class_constraints_mixin() {
    check(
        r#"
sealed class A {}

class B extends A {}

mixin M on A {}

void f(A x) {
  switch (x) {
//^^^^^^
// [diag.nonExhaustiveSwitchStatement] The type 'A' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'M()'.
    case B _:
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_sealed_class_has_extension_type_1of1() {
    check(
        r#"
sealed class A {}
class B extends A {}
extension type EA(A it) implements A {}

void f(A x) {
  switch (x) {
    case B():
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_sealed_class_has_extension_type_1of2() {
    check(
        r#"
sealed class A {}
class B extends A {}
class C extends A {}
extension type EA(A it) implements A {}

void f(A x) {
  switch (x) {
//^^^^^^
// [diag.nonExhaustiveSwitchStatement] The type 'A' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'C()'.
    case B():
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_sealed_class_implemented_by_enum_3at2() {
    check(
        r#"
sealed class A {}

class B implements A {}

enum E implements A {
  a, b
}

void f(A x) {
  switch (x) {
//^^^^^^
// [diag.nonExhaustiveSwitchStatement] The type 'A' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'E.b'.
    case B _:
    case E.a:
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_sealed_class_implemented_by_enum_3at3() {
    check(
        r#"
sealed class A {}

class B implements A {}

enum E implements A {
  a, b
}

void f(A x) {
  switch (x) {
    case B _:
    case E.a:
    case E.b:
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_sealed_class_implemented_by_mixin_2at1() {
    check(
        r#"
sealed class A {}

class B implements A {}

mixin M implements A {}

void f(A x) {
  switch (x) {
//^^^^^^
// [diag.nonExhaustiveSwitchStatement] The type 'A' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'M()'.
    case B _:
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_sealed_class_implemented_by_mixin_2at2() {
    check(
        r#"
sealed class A {}

class B implements A {}

mixin M implements A {}

void f(A x) {
  switch (x) {
    case B _:
    case M _:
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_sealed_class_unresolved_identifier() {
    check(
        r#"
sealed class A {}
class B extends A {}

void f(A x) {
  switch (x) {
    case unresolved:
//       ^^^^^^^^^^
// [diag.undefinedIdentifier] Undefined name 'unresolved'.
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_sealed_class_unresolved_object() {
    check(
        r#"
sealed class A {}
class B extends A {}

void f(A x) {
  switch (x) {
    case Unresolved():
//       ^^^^^^^^^^
// [diag.undefinedClass] Undefined class 'Unresolved'.
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_type_variable_bound_bool_true() {
    check(
        r#"
void f<T extends bool>(T x) {
  switch (x) {
//^^^^^^
// [diag.nonExhaustiveSwitchStatement] The type 'T' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'false'.
    case true:
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_type_variable_bound_bool_true_false() {
    check(
        r#"
void f<T extends bool>(T x) {
  switch (x) {
    case true:
    case false:
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_type_variable_promoted_bool_true() {
    check(
        r#"
void f<T>(T x) {
  if (x is bool) {
    switch (x) {
//  ^^^^^^
// [diag.nonExhaustiveSwitchStatement] The type 'T & bool' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'false'.
      case true:
        break;
    }
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_always_exhaustive_type_variable_promoted_bool_true_false() {
    check(
        r#"
void f<T>(T x) {
  if (x is bool) {
    switch (x) {
      case true:
      case false:
        break;
    }
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_invalid_type_empty() {
    check(
        r#"
void f(Unresolved x) {
//     ^^^^^^^^^^
// [diag.undefinedClass] Undefined class 'Unresolved'.
  switch (x) {}
}
"#,
        &[],
    );
}

#[test]
fn statement_not_always_exhaustive_int() {
    check(
        r#"
void f(int x) {
  switch (x) {
    case 0:
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn statement_private_enum() {
    check(
        r#"
import 'private_enum.dart';

void f() {
  switch (e()) {
//^^^^^^
// [diag.nonExhaustiveSwitchStatementPrivate] The enum '_E' isn't exhaustively matched by the switch cases because some of the enum constants are private.
  }
}
"#,
        &[(
            "private_enum.dart",
            r#"
enum _E { a, b }
_E e() => _E.a;
"#,
        )],
    );
}

#[test]
fn statement_private_enum_constant() {
    check(
        r#"
import 'private_enum.dart';

void f(E e) {
  switch (e) {
//^^^^^^
// [diag.nonExhaustiveSwitchStatement] The type 'E' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'E.b'.
    case E.a:
      break;
  }
}
"#,
        &[(
            "private_enum.dart",
            r#"
enum E { a, b, _c }
"#,
        )],
    );
}

#[test]
fn statement_private_enum_constant_only() {
    check(
        r#"
import 'private_enum.dart';

void f(E e) {
  switch (e) {
//^^^^^^
// [diag.nonExhaustiveSwitchStatementPrivate] The enum 'E' isn't exhaustively matched by the switch cases because some of the enum constants are private.
    case E.a:
    case E.b:
      break;
  }
}
"#,
        &[(
            "private_enum.dart",
            r#"
enum E { a, b, _c }
"#,
        )],
    );
}

#[test]
fn statement_private_sealed() {
    check(
        r#"
import 'private_sealed.dart';

Object f() {
  switch (a()) {
//^^^^^^
// [diag.nonExhaustiveSwitchStatement] The type '_A' isn't exhaustively matched by the switch cases since it doesn't match the pattern 'B()'.
  }
}
"#,
        &[(
            "private_sealed.dart",
            r#"
sealed class _A {}
class B extends _A {}
_A a() => B();
"#,
        )],
    );
}

#[test]
fn unreachable_case_expression_bool_false_true_false() {
    check(
        r#"
Object f(bool x) {
  return switch (x) {
    false => 0,
    true => 1,
    false => 2,
//        ^^
// [diag.unreachableSwitchCase] This case is covered by the previous cases.
  };
}
"#,
        &[],
    );
}

#[test]
fn unreachable_case_expression_bool_wildcard_true_false() {
    check(
        r#"
Object f(bool x) {
  return switch (x) {
    _ => 0,
    true => 1,
//  ^^^^^^^^^
// [diag.deadCode] Dead code.
//       ^^
// [diag.unreachableSwitchCase] This case is covered by the previous cases.
    false => 2,
//  ^^^^^^^^^^
// [diag.deadCode] Dead code.
//        ^^
// [diag.unreachableSwitchCase] This case is covered by the previous cases.
  };
}
"#,
        &[],
    );
}

#[test]
fn unreachable_case_expression_guarded_reachable() {
    check(
        r#"
enum E { e1, e2 }
Object f(E e, bool b) => switch (e) {
  E.e1 when b => 0,
  E.e2 => 1,
  E.e1 => 2,
};
"#,
        &[],
    );
}

#[test]
fn unreachable_case_expression_guarded_unreachable() {
    check(
        r#"
enum E { e1, e2 }
Object f(E e, bool b) => switch (e) {
  E.e1 => 0,
  E.e2 => 1,
  E.e1 when b => 2,
//            ^^
// [diag.unreachableSwitchCase] This case is covered by the previous cases.
};
"#,
        &[],
    );
}

#[test]
fn unreachable_case_expression_unresolved_wildcard() {
    check(
        r#"
int f(Object? x) {
  return switch (x) {
    Unresolved() => 0,
//  ^^^^^^^^^^
// [diag.undefinedClass] Undefined class 'Unresolved'.
    _ => -1,
  };
}
"#,
        &[],
    );
}

#[test]
fn unreachable_case_statement_bool() {
    check(
        r#"
void f(bool x) {
  switch (x) {
    case false:
    case true:
    case false:
//  ^^^^
// [diag.unreachableSwitchCase] This case is covered by the previous cases.
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn unreachable_case_statement_const_unresolved_identifier_const() {
    check(
        r#"
void f(Object? x) {
  switch (x) {
    case 0:
      break;
    case unresolved:
//       ^^^^^^^^^^
// [diag.undefinedIdentifier] Undefined name 'unresolved'.
      break;
    case 2:
      break;
  };
}
"#,
        &[],
    );
}

#[test]
fn unreachable_case_statement_const_unresolved_object_const() {
    check(
        r#"
void f(Object? x) {
  switch (x) {
    case 0:
      break;
    case Unresolved():
//       ^^^^^^^^^^
// [diag.undefinedClass] Undefined class 'Unresolved'.
      break;
    case 2:
      break;
  };
}
"#,
        &[],
    );
}

#[test]
fn unreachable_case_statement_guarded_reachable() {
    check(
        r#"
enum E { e1, e2 }
void f(E e, bool b) {
  switch (e) {
    case E.e1 when b:
      break;
    case E.e2:
      break;
    case E.e1:
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn unreachable_case_statement_guarded_unreachable() {
    check(
        r#"
enum E { e1, e2 }
void f(E e, bool b) {
  switch (e) {
    case E.e1:
      break;
    case E.e2:
      break;
    case E.e1 when b:
//  ^^^^
// [diag.unreachableSwitchCase] This case is covered by the previous cases.
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn unreachable_case_statement_type_check_exact() {
    check(
        r#"
void f(int x) {
  switch (x) {
    case int():
      break;
    case int():
//  ^^^^
// [diag.deadCode] Dead code.
// [diag.unreachableSwitchCase] This case is covered by the previous cases.
    case int():
//  ^^^^
// [diag.deadCode] Dead code.
// [diag.unreachableSwitchCase] This case is covered by the previous cases.
      break;
//    ^^^^^^
// [diag.deadCode] Dead code.
  }
}
"#,
        &[],
    );
}

#[test]
fn unreachable_default_bool() {
    check(
        r#"
void f(bool x) {
  switch (x) {
    case false:
    case true:
    default:
//  ^^^^^^^
// [diag.unreachableSwitchDefault] This default clause is covered by the previous cases.
      break;
  }
}
"#,
        &[],
    );
}

#[test]
fn unreachable_default_enum() {
    check(
        r#"
enum E { e1, e2 }

String f(E e) {
  switch (e) {
    case E.e1:
      return 'e1';
    case E.e2:
      return 'e2';
    default:
//  ^^^^^^^
// [diag.unreachableSwitchDefault] This default clause is covered by the previous cases.
      return 'Some other value of E (impossible)';
  }
}
"#,
        &[],
    );
}

#[test]
fn unreachable_default_not_always_exhaustive() {
    check(
        r#"
String f(List x) {
  switch (x) {
    case []:
      return 'empty';
    case [var y, ...]:
      return 'non-empty starting with $y';
    default:
      return 'impossible';
  }
}
"#,
        &[],
    );
}

#[test]
fn unreachable_default_sealed_class() {
    check(
        r#"
sealed class A {}
class B extends A {}
class C extends A {}

String f(A x) {
  switch (x) {
    case B():
      return 'B';
    case C():
      return 'C';
    default:
//  ^^^^^^^
// [diag.unreachableSwitchDefault] This default clause is covered by the previous cases.
      return 'Some other subclass of A (impossible)';
  }
}
"#,
        &[],
    );
}

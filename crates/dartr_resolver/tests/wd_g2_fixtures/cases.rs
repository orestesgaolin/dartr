// Generated from the analyzer diagnostic tests by the wave D test generator (do not edit).
use crate::support::g3::Ported;

pub const CASES: &[Ported] = &[
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_cascaded_deadCode"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  never..=> 1;
//     ^^^^^^^
// [diag.deadCode] Dead code.
}

Never get never => throw 0;
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_nullaware_deadCode"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  null?.=> 1;
//    ^^^^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_nullawareCascaded_deadCode"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  null?..=> 1;
//    ^^^^^^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_parameterized_cascaded_deadCode"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  never..(_) => 1;
//     ^^^^^^^^^^^
// [diag.deadCode] Dead code.
}

Never get never => throw 0;
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_parameterized_nullaware_deadCode"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  null?.(_) => 1;
//    ^^^^^^^^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_parameterized_nullawareCascaded_deadCode"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  null?..(_) => 1;
//    ^^^^^^^^^^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_parameterized_plain_deadCode"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  never.(_) => 1;
//     ^^^^^^^^^^
// [diag.deadCode] Dead code.
}

Never get never => throw 0;
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_afterForEachWithBreakLabel"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f(List<Object> values) {
  named: {
    for (var x in values) {
      if (x == 42) {
        break named;
      }
    }
    return;
  }
  print('not dead');
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_afterForWithBreakLabel"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  named: {
    for (int i = 0; i < 7; i++) {
      if (i == 42)
        break named;
    }
    return;
  }
  print('not dead');
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_afterTryCatch"#,
        strict_inference: false,
        packages: &[],
        source: r#"
main() {
  try {
    return f();
  } catch (e) {
    print(e);
  }
  print('not dead');
}
f() {
  throw 'foo';
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_assert"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  return;
  assert (true);
//^^^^^^^^^^^^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_assigned_methodInvocation"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  int? i = 1;
  i?.truncate();
// ^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?.' is unnecessary.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_constructorInitializerWithThrow_thenBlockBody"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  int x;
  A() : x = throw 0 {
// [diag.deadCode][column 21][length 12] Dead code.
    x;
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_constructorInitializerWithThrow_thenEmptyBlockBody"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  int x;
  A() : x = throw 0 {}
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_constructorInitializerWithThrow_thenEmptyBody"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  int x;
  A() : x = throw 0;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_constructorInitializerWithThrow_thenExpressions"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  var x = [8];
  A() : x = [7, throw 8, 9];
//                       ^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_constructorInitializerWithThrow_thenInitializer"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  int x;
  int y;
  A()
      : x = throw 0,
        y = 7;
//      ^^^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_continueInSwitch"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(int i) {
  for (;; 1) {
    switch (i) {
      default:
        continue;
    }
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_deadBlock_conditionalElse"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  true ? 1 : 2;
//           ^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_deadBlock_conditionalElse_debugConst"#,
        strict_inference: false,
        packages: &[],
        source: r#"
const bool DEBUG = true;
f() {
  DEBUG ? 1 : 2;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_deadBlock_conditionalThen"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  false ? 1 : 2;
//        ^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_deadBlock_conditionalThen_debugConst"#,
        strict_inference: false,
        packages: &[],
        source: r#"
const bool DEBUG = false;
f() {
  DEBUG ? 1 : 2;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_deadBlock_else"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  if(true) {} else {}
//                 ^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_deadBlock_else_debugConst"#,
        strict_inference: false,
        packages: &[],
        source: r#"
const bool DEBUG = true;
f() {
  if(DEBUG) {} else {}
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_deadBlock_if"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  if(false) {}
//          ^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_deadBlock_if_debugConst_prefixedIdentifier"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  static const bool DEBUG = false;
}
f() {
  if(A.DEBUG) {}
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_deadBlock_if_debugConst_prefixedIdentifier2"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib2.dart"#,
                r#"
class A {
  static const bool DEBUG = false;
}"#,
            )],
        )],
        source: r#"
import 'lib2.dart';
f() {
  if(A.DEBUG) {}
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_deadBlock_if_debugConst_propertyAccessor"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib2.dart"#,
                r#"
class A {
  static const bool DEBUG = false;
}
"#,
            )],
        )],
        source: r#"
import 'lib2.dart' as LIB;
f() {
  if(LIB.A.DEBUG) {}
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_deadBlock_if_debugConst_simpleIdentifier"#,
        strict_inference: false,
        packages: &[],
        source: r#"
const bool DEBUG = false;
f() {
  if(DEBUG) {}
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_deadBlock_ifElement"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  [
    if (false) 2,
//             ^
// [diag.deadCode] Dead code.
  ];
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_deadBlock_ifElement_else"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  [
    if (true) 2
    else 3,
//       ^
// [diag.deadCode] Dead code.
  ];
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_deadBlock_while"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  while(false) {}
//             ^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_deadBlock_while_debugConst"#,
        strict_inference: false,
        packages: &[],
        source: r#"
const bool DEBUG = false;
f() {
  while(DEBUG) {}
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_deadCatch_catchFollowingCatch"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {}
f() {
  try {} catch (e) {} catch (e) {}
//                    ^^^^^^^^^^^^
// [diag.deadCodeCatchFollowingCatch] Dead code: Catch clauses after a 'catch (e)' or an 'on Object catch (e)' are never reached.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_deadCatch_catchFollowingCatch_object"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  try {} on Object catch (e) {} catch (e) {}
//                        ^
// [diag.unusedCatchClause] The exception variable 'e' isn't used, so the 'catch' clause can be removed.
//                              ^^^^^^^^^^^^
// [diag.deadCodeCatchFollowingCatch] Dead code: Catch clauses after a 'catch (e)' or an 'on Object catch (e)' are never reached.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_deadCatch_onCatchSubtype"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {}
class B extends A {}
f() {
  try {} on A catch (e) {} on B catch (e) {}
//                   ^
// [diag.unusedCatchClause] The exception variable 'e' isn't used, so the 'catch' clause can be removed.
//                         ^^^^^^^^^^^^^^^^^
// [diag.deadCodeOnCatchSubtype] Dead code: This on-catch block won't be executed because 'B' is a subtype of 'A' and hence will have been caught already.
//                                     ^
// [diag.unusedCatchClause] The exception variable 'e' isn't used, so the 'catch' clause can be removed.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_deadCatch_onCatchSupertype"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {}
class B extends A {}
f() {
  try {} on B catch (e) {} on A catch (e) {} catch (e) {}
//                   ^
// [diag.unusedCatchClause] The exception variable 'e' isn't used, so the 'catch' clause can be removed.
//                                     ^
// [diag.unusedCatchClause] The exception variable 'e' isn't used, so the 'catch' clause can be removed.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_deadOperandLHS_and"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  bool b = false && false;
//               ^^^^^^^^
// [diag.deadCode] Dead code.
  print(b);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_deadOperandLHS_and_debugConst"#,
        strict_inference: false,
        packages: &[],
        source: r#"
const bool DEBUG = false;
f() {
  bool b = DEBUG && false;
  print(b);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_deadOperandLHS_and_nested"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  bool b = false && (false && false);
//               ^^^^^^^^^^^^^^^^^^^
// [diag.deadCode] Dead code.
  print(b);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_deadOperandLHS_or"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  bool b = true || true;
//              ^^^^^^^
// [diag.deadCode] Dead code.
  print(b);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_deadOperandLHS_or_debugConst"#,
        strict_inference: false,
        packages: &[],
        source: r#"
const bool DEBUG = true;
f() {
  bool b = DEBUG || true;
//     ^
// [diag.unusedLocalVariable] The value of the local variable 'b' isn't used.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_deadOperandLHS_or_nested"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  bool b = true || (false && false);
//              ^^^^^^^^^^^^^^^^^^^
// [diag.deadCode] Dead code.
  print(b);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_documentationComment"#,
        strict_inference: false,
        packages: &[],
        source: r#"
/// text
int f() => 0;
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_doWhile"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(bool c) {
  do {
    print(c);
    return;
  } while (c);
//^^^^^^^^^^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_doWhile_break"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(bool c) {
  do {
    if (c) {
     break;
    }
    return;
  } while (c);
//^^^^^^^^^^^^
// [diag.deadCode] Dead code.
  print('');
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_doWhile_break_doLabel"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(bool c) {
  label:
  do {
    if (c) {
      break label;
    }
    return;
  } while (c);
//^^^^^^^^^^^^
// [diag.deadCode] Dead code.
  print('');
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_doWhile_statements"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(bool c) {
  do {
    print(c);
    return;
  } while (c);
//^^^^^^^^^^^^
// [diag.deadCode] Dead code.
  print('2');
//^^^^^^^^^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_block_forStatement_updaters"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  for (;; 1) {
//        ^
// [diag.deadCode] Dead code.
    return;
    2;
//  ^^
// [diag.deadCode] Dead code.
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_block_forStatement_updaters_multiple"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  for (;; 1, 2) {
//        ^^^^
// [diag.deadCode] Dead code.
    return;
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forElementParts_condition_exists"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() => [for (; throw 0; 1) 0];
//                      ^
// [diag.deadCode] Dead code.
//                         ^^^
// [diag.deadCode] Dead code.
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forElementParts_condition_throw"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f(bool Function(Object?, Object?) g) => [for (; g(throw 0, 1); 2) 0];
//                                                         ^^^^^^^^^^
// [diag.deadCode] Dead code.
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forElementParts_initializer_declaration_throw"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() => [for (var i = throw 0; true; 1) 0];
//               ^
// [diag.unusedLocalVariable] The value of the local variable 'i' isn't used.
//                            ^^^^^^^
// [diag.deadCode] Dead code.
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forElementParts_initializer_expression_throw"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() => [for (throw 0; true; 1) 0];
//                    ^^^^^^^
// [diag.deadCode] Dead code.
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forElementParts_updaters_assignmentExpression"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() => [for (var i = 0;; i = i + 1) throw ''];
//                       ^^^^^^^^^
// [diag.deadCode] Dead code.
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forElementParts_updaters_binaryExpression"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() => [for (var i = 0;; i + 1) throw ''];
//                       ^^^^^
// [diag.deadCode] Dead code.
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forElementParts_updaters_cascadeExpression"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() => [for (var i = 0;; i..sign) throw ''];
//                       ^^^^^^^
// [diag.deadCode] Dead code.
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forElementParts_updaters_conditionalExpression"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() => [for (var i = 0;; i > 1 ? i : i) throw ''];
//                       ^^^^^^^^^^^^^
// [diag.deadCode] Dead code.
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forElementParts_updaters_indexExpression"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f(List<int> values) => [for (;; values[0]) throw ''];
//                              ^^^^^^^^^
// [diag.deadCode] Dead code.
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forElementParts_updaters_instanceCreationExpression"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class C {}
f() => [for (;; C()) throw ''];
//              ^^^
// [diag.deadCode] Dead code.
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forElementParts_updaters_methodInvocation"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() => [for (var i = 0;; i.toString()) throw ''];
//                       ^^^^^^^^^^^^
// [diag.deadCode] Dead code.
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forElementParts_updaters_postfixExpression"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() => [for (var i = 0;; i++) throw ''];
//                       ^^^
// [diag.deadCode] Dead code.
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forElementParts_updaters_prefixedIdentifier"#,
        strict_inference: false,
        packages: &[],
        source: r#"
import 'dart:math' as m;

f() => [for (;; m.Point) throw ''];
//              ^^^^^^^
// [diag.deadCode] Dead code.
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forElementParts_updaters_prefixExpression"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() => [for (var i = 0;; ++i) throw ''];
//                       ^^^
// [diag.deadCode] Dead code.
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forElementParts_updaters_propertyAccess"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() => [for (var i = 0;; (i).sign) throw ''];
//                       ^^^^^^^^
// [diag.deadCode] Dead code.
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forElementParts_updaters_throw"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() => [for (;; 0, throw 1, 2) 0];
//                          ^
// [diag.deadCode] Dead code.
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forParts_condition_exists"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  for (; throw 0; 1) {}
//                ^
// [diag.deadCode] Dead code.
//                   ^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forParts_condition_throw"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(bool Function(Object?, Object?) g) {
  for (; g(throw 0, 1); 2) {}
//                  ^^^^^^^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forParts_initializer_declaration_throw"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  for (var i = throw 0; true; 1) {}
//         ^
// [diag.unusedLocalVariable] The value of the local variable 'i' isn't used.
//                      ^^^^^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forParts_initializer_expression_throw"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  for (throw 0; true; 1) {}
//              ^^^^^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forParts_updaters_assignmentExpression"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  for (var i = 0;; i = i + 1) {
//                 ^^^^^^^^^
// [diag.deadCode] Dead code.
    return;
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forParts_updaters_binaryExpression"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  for (var i = 0;; i + 1) {
//                 ^^^^^
// [diag.deadCode] Dead code.
    return;
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forParts_updaters_cascadeExpression"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  for (var i = 0;; i..sign) {
//                 ^^^^^^^
// [diag.deadCode] Dead code.
    return;
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forParts_updaters_conditionalExpression"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  for (var i = 0;; i > 1 ? i : i) {
//                 ^^^^^^^^^^^^^
// [diag.deadCode] Dead code.
    return;
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forParts_updaters_indexExpression"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(List<int> values) {
  for (;; values[0]) {
//        ^^^^^^^^^
// [diag.deadCode] Dead code.
    return;
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forParts_updaters_instanceCreationExpression"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class C {}
void f() {
  for (;; C()) {
//        ^^^
// [diag.deadCode] Dead code.
    return;
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forParts_updaters_methodInvocation"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  for (var i = 0;; i.toString()) {
//                 ^^^^^^^^^^^^
// [diag.deadCode] Dead code.
    return;
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forParts_updaters_postfixExpression"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  for (var i = 0;; i++) {
//                 ^^^
// [diag.deadCode] Dead code.
    return;
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forParts_updaters_prefixedIdentifier"#,
        strict_inference: false,
        packages: &[],
        source: r#"
import 'dart:math' as m;

void f() {
  for (;; m.Point) {
//        ^^^^^^^
// [diag.deadCode] Dead code.
    return;
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forParts_updaters_prefixExpression"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  for (var i = 0;; ++i) {
//                 ^^^
// [diag.deadCode] Dead code.
    return;
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forParts_updaters_propertyAccess"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  for (var i = 0;; (i).sign) {
//                 ^^^^^^^^
// [diag.deadCode] Dead code.
    return;
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forParts_updaters_throw"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  for (;; 0, throw 1, 2) {}
//                    ^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_forStatement"#,
        strict_inference: false,
        packages: &[],
        source: r#"
main() {
  for (var v in [0, 1, 2]) {
    v;
    return;
    1;
//  ^^
// [diag.deadCode] Dead code.
  }
  2;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_ifStatement"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(bool a) {
  if (a) {
    return;
    1;
//  ^^
// [diag.deadCode] Dead code.
  }
  2;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_list_forElement_updaters"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() => [for (;; 1) ...[throw '', 2]];
//              ^
// [diag.deadCode] Dead code.
//                               ^^^^
// [diag.deadCode] Dead code.
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_list_forElement_updaters_multiple"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() => [for (;; 1, 2) ...[throw '']];
//              ^^^^
// [diag.deadCode] Dead code.
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_nestedBlock_forStatement_updaters"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  for (;; 1) {
//        ^
// [diag.deadCode] Dead code.
    {
      return;
      2;
// [diag.deadCode][column 7][length 8] Dead code.
    }
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_nestedBlock_forStatement_updaters_multiple"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  for (;; 1, 2) {
//        ^^^^
// [diag.deadCode] Dead code.
    {
      return;
    }
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_tryStatement_body"#,
        strict_inference: false,
        packages: &[],
        source: r#"
Never foo() => throw 0;

main() {
  try {
    foo();
    1;
//  ^^
// [diag.deadCode] Dead code.
  } catch (_) {
    2;
  }
  3;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_tryStatement_catchClause"#,
        strict_inference: false,
        packages: &[],
        source: r#"
main() {
  try {
    1;
  } catch (_) {
    return;
    2;
//  ^^
// [diag.deadCode] Dead code.
  }
  3;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_flowEnd_tryStatement_finally"#,
        strict_inference: false,
        packages: &[],
        source: r#"
main() {
  try {
    1;
  } finally {
    2;
    return;
    3;
// [diag.deadCode][column 5][length 11] Dead code.
  }
  4;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_forStatement"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  return;
  for (;;) {}
//^^^^^^^^^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_ifStatement_noCase_conditionFalse"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  if (false) {
// [diag.deadCode][column 14][length 12] Dead code.
    1;
  } else {
    2;
  }
  3;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_ifStatement_noCase_conditionTrue"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  if (true) {
    1;
  } else {
// [diag.deadCode][column 10][length 12] Dead code.
    2;
  }
  3;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_invokeNever_functionExpressionInvocation_getter_propertyAccess"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  Never get f => throw 0;
}
void g(A a) {
  a.f(0);
//^^^
// [diag.receiverOfTypeNever] The receiver is of type 'Never', and will never complete with a value.
// [diag.deadCode][column 6][length 16] Dead code.
  print(1);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_invokeNever_functionExpressionInvocation_parenthesizedExpression"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void g(Never f) {
  (f)(0);
//^^^
// [diag.receiverOfTypeNever] The receiver is of type 'Never', and will never complete with a value.
// [diag.deadCode][column 6][length 16] Dead code.
  print(1);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_invokeNever_functionExpressionInvocation_simpleIdentifier"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void g(Never f) {
  f(0);
//^
// [diag.receiverOfTypeNever] The receiver is of type 'Never', and will never complete with a value.
// [diag.deadCode][column 4][length 16] Dead code.
  print(1);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_lateWildCardVariable_initializer"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  late var _ = 0;
//             ^
// [diag.deadCodeLateWildcardVariableInitializer] Dead code: The assigned-to wildcard variable is marked late and can never be referenced so this initializer will never be evaluated.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_lateWildCardVariable_noInitializer"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  late var _;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_notUnassigned_propertyAccess"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(int? i) {
  (i)?.sign;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_potentiallyAssigned_propertyAccess"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(bool b) {
  int? i;
  if (b) {
    i = 1;
  }
  (i)?.sign;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_returnTypeNever_function"#,
        strict_inference: false,
        packages: &[],
        source: r#"
Never foo() => throw 0;

main() {
  foo();
  1;
//^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_returnTypeNever_getter"#,
        strict_inference: false,
        packages: &[],
        source: r#"
Never get foo => throw 0;

main() {
  foo;
  2;
//^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_statementAfterAlwaysThrowsGetter"#,
        strict_inference: false,
        packages: &[],
        source: r#"
import 'package:meta/meta.dart';

class C {
  @alwaysThrows
// ^^^^^^^^^^^^
// [diag.deprecatedMemberUseWithMessage] 'alwaysThrows' is deprecated and shouldn't be used. Use a return type of 'Never' instead.
  int get a {
    throw 'msg';
  }
}

f() {
  print(1);
  new C().a;
  print(2);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_statementAfterBreak_inDefaultCase"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f(v) {
  switch(v) {
    case 1:
    default:
      break;
      print(1);
//    ^^^^^^^^^
// [diag.deadCode] Dead code.
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_statementAfterBreak_inForEachStatement"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  var list;
  for(var l in list) {
    break;
    print(l);
//  ^^^^^^^^^
// [diag.deadCode] Dead code.
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_statementAfterBreak_inForStatement"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  for(;;) {
    break;
    print(1);
//  ^^^^^^^^^
// [diag.deadCode] Dead code.
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_statementAfterBreak_inSwitchCase"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f(v) {
  switch(v) {
    case 1:
      break;
      print(1);
//    ^^^^^^^^^
// [diag.deadCode] Dead code.
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_statementAfterBreak_inWhileStatement"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f(v) {
  while(v) {
    break;
    print(1);
//  ^^^^^^^^^
// [diag.deadCode] Dead code.
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_statementAfterContinue_inForEachStatement"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  var list;
  for(var l in list) {
    continue;
    print(l);
//  ^^^^^^^^^
// [diag.deadCode] Dead code.
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_statementAfterContinue_inForStatement"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  for(;;) {
    continue;
    print(1);
//  ^^^^^^^^^
// [diag.deadCode] Dead code.
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_statementAfterContinue_inWhileStatement"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f(v) {
  while(v) {
    continue;
    print(1);
//  ^^^^^^^^^
// [diag.deadCode] Dead code.
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_statementAfterExitingIf_returns"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  if (1 > 2) {
    return;
  } else {
    return;
  }
  print(1);
//^^^^^^^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_statementAfterIfWithoutElse"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  if (1 < 0) {
    return;
  }
  print(1);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_statementAfterRethrow"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  try {
    print(1);
  } catch (e) {
    rethrow;
    print(2);
//  ^^^^^^^^^
// [diag.deadCode] Dead code.
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_statementAfterReturn_function"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  print(1);
  return;
  print(2);
//^^^^^^^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_statementAfterReturn_function_local"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  void g() {
    print(1);
    return;
    print(2);
//  ^^^^^^^^^
// [diag.deadCode] Dead code.
  }
  g();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_statementAfterReturn_functionExpression"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  () {
    print(1);
    return;
    print(2);
//  ^^^^^^^^^
// [diag.deadCode] Dead code.
  };
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_statementAfterReturn_ifStatement"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f(bool b) {
  if(b) {
    print(1);
    return;
    print(2);
//  ^^^^^^^^^
// [diag.deadCode] Dead code.
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_statementAfterReturn_method"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  m() {
    print(1);
    return;
    print(2);
//  ^^^^^^^^^
// [diag.deadCode] Dead code.
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_statementAfterReturn_nested"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  print(1);
  return;
  if(false) {}
//^^^^^^^^^^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_statementAfterReturn_twoReturns"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  print(1);
  return;
  print(2);
// [diag.deadCode][column 3][length 31] Dead code.
  return;
  print(3);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_statementAfterThrow"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  print(1);
  throw 'Stop here';
  print(2);
//^^^^^^^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_switchCase_final_break"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(int a) {
  switch (a) {
    case 0:
      try {} finally {
        return;
      }
      break;
//    ^^^^^^
// [diag.deadCode] Dead code.
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_switchCase_final_continue"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(int a) {
  for (var i = 0; i < 2; i++) {
    switch (a) {
      case 0:
        try {} finally {
          return;
        }
        continue;
//      ^^^^^^^^^
// [diag.deadCode] Dead code.
    }
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_switchCase_final_rethrow"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(int a) {
  try {
    // empty
  } on int {
    switch (a) {
      case 0:
        try {} finally {
          return;
        }
        rethrow;
//      ^^^^^^^^
// [diag.deadCode] Dead code.
    }
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_switchCase_final_return"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(int a) {
  switch (a) {
    case 0:
      try {} finally {
        return;
      }
      return;
//    ^^^^^^^
// [diag.deadCode] Dead code.
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_switchCase_final_throw"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(int a) {
  switch (a) {
    case 0:
      try {} finally {
        return;
      }
      throw 0;
//    ^^^^^^^^
// [diag.deadCode] Dead code.
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_switchStatement_exhaustive"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum Foo { a, b }

int f(Foo foo) {
  switch (foo) {
    case Foo.a: return 0;
    case Foo.b: return 1;
  }
  return -1;
//^^^^^^^^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_unassigned_cascadeExpression_indexExpression"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  List<int>? l;
  l?..[0]..length;
// ^^^^^^^^^^^^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_unassigned_cascadeExpression_methodInvocation"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  int? i;
  i?..toInt()..isEven;
// ^^^^^^^^^^^^^^^^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_unassigned_cascadeExpression_propertyAccess"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  int? i;
  i?..sign..isEven;
// ^^^^^^^^^^^^^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_unassigned_indexExpression"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  List<int>? l;
  l?[0];
// ^^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_unassigned_indexExpression_indexExpression"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  List<List<int>>? l;
  l?[0][0];
// ^^^^^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_unassigned_methodInvocation"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  int? i;
  i?.truncate();
// ^^^^^^^^^^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_unassigned_methodInvocation_methodInvocation"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  int? i;
  i?.truncate().truncate();
// ^^^^^^^^^^^^^^^^^^^^^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_unassigned_methodInvocation_propertyAccess"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  int? i;
  i?.truncate().sign;
// ^^^^^^^^^^^^^^^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_unassigned_propertyAccess"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  int? i;
  (i)?.sign;
//   ^^^^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_unassigned_propertyAccess_propertyAccess"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  int? i;
  (i)?.sign.sign;
//   ^^^^^^^^^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"dead_code_test.dart::DeadCodeTest_AnonymousMethodsExperiment::test_yield"#,
        strict_inference: false,
        packages: &[],
        source: r#"
Iterable<int> f() sync* {
  return;
  yield 1;
//^^^^^^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"duplicate_hidden_name_test.dart::DuplicateHiddenNameTest::test_library_hidden"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
class A {}
class B {}
"#,
            )],
        )],
        source: r#"
export 'lib1.dart' hide A, B, A;
//                            ^
// [diag.duplicateHiddenName] Duplicate hidden name.
"#,
        expected: None,
    },
    Ported {
        name: r#"duplicate_import_test.dart::DuplicateExportTest::test_library_duplicateExport"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
class A {}
class B {}
"#,
            )],
        )],
        source: r#"
export 'lib1.dart';
export 'lib1.dart';
//     ^^^^^^^^^^^
// [diag.duplicateExport] Duplicate export.
"#,
        expected: None,
    },
    Ported {
        name: r#"duplicate_import_test.dart::DuplicateExportTest::test_library_duplicateExport_differentShow"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
class A {}
class B {}
"#,
            )],
        )],
        source: r#"
export 'lib1.dart' show A;
export 'lib1.dart' show B;
"#,
        expected: None,
    },
    Ported {
        name: r#"duplicate_import_test.dart::DuplicateExportTest::test_library_duplicateExport_sameShow"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
class A {}
class B {}
"#,
            )],
        )],
        source: r#"
export 'lib1.dart' show A;
export 'lib1.dart' show A;
//     ^^^^^^^^^^^
// [diag.duplicateExport] Duplicate export.
"#,
        expected: None,
    },
    Ported {
        name: r#"duplicate_import_test.dart::DuplicateImportTest::test_library_duplicateImport_absolute_absolute"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"a.dart"#,
                r#"
class A {}
"#,
            )],
        )],
        source: r#"
import 'package:test/a.dart';
import 'package:test/a.dart';
//     ^^^^^^^^^^^^^^^^^^^^^
// [diag.duplicateImport] Duplicate import.

final a = A();
"#,
        expected: None,
    },
    Ported {
        name: r#"duplicate_import_test.dart::DuplicateImportTest::test_library_duplicateImport_relative_absolute"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"a.dart"#,
                r#"
class A {}
"#,
            )],
        )],
        source: r#"
import 'a.dart';
import 'package:test/a.dart';
//     ^^^^^^^^^^^^^^^^^^^^^
// [diag.duplicateImport] Duplicate import.

final a = A();
"#,
        expected: None,
    },
    Ported {
        name: r#"duplicate_import_test.dart::DuplicateImportTest::test_library_duplicateImport_relative_relative"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"a.dart"#,
                r#"
class A {}
"#,
            )],
        )],
        source: r#"
import 'a.dart';
import 'a.dart';
//     ^^^^^^^^
// [diag.duplicateImport] Duplicate import.

final a = A();
"#,
        expected: None,
    },
    Ported {
        name: r#"duplicate_shown_name_test.dart::DuplicateShownNameTest::test_library_shown"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
class A {}
class B {}
"#,
            )],
        )],
        source: r#"
export 'lib1.dart' show A, B, A;
//                            ^
// [diag.duplicateShownName] Duplicate shown name.
"#,
        expected: None,
    },
    Ported {
        name: r#"todo_test.dart::TodoTest::test_eof"#,
        strict_inference: false,
        packages: &[],
        source: r#"
main() {}
// TODO: Implement something else
// ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.todo] TODO: Implement something else
"#,
        expected: None,
    },
    Ported {
        name: r#"todo_test.dart::TodoTest::test_fixme"#,
        strict_inference: false,
        packages: &[],
        source: r#"
main() {
  // FIXME: Implement
//   ^^^^^^^^^^^^^^^^
// [diag.fixme] FIXME: Implement
}
"#,
        expected: None,
    },
    Ported {
        name: r#"todo_test.dart::TodoTest::test_hack"#,
        strict_inference: false,
        packages: &[],
        source: r#"
main() {
  // HACK: This is a hack
//   ^^^^^^^^^^^^^^^^^^^^
// [diag.hack] HACK: This is a hack
}
"#,
        expected: None,
    },
    Ported {
        name: r#"todo_test.dart::TodoTest::test_todo_multiLineComment"#,
        strict_inference: false,
        packages: &[],
        source: r#"
main() {
  /* TODO: Implement */
//   ^^^^^^^^^^^^^^^
// [diag.todo] TODO: Implement
  /* TODO: Implement*/
//   ^^^^^^^^^^^^^^^
// [diag.todo] TODO: Implement
}
"#,
        expected: None,
    },
    Ported {
        name: r#"todo_test.dart::TodoTest::test_todo_multiLineComment2"#,
        strict_inference: false,
        packages: &[],
        source: r#"
main() {
/*
TODO: Implement1
// [diag.todo][column 1][length 16] TODO: Implement1
TODO: Implement2
// [diag.todo][column 1][length 16] TODO: Implement2
*/
}
"#,
        expected: None,
    },
    Ported {
        name: r#"todo_test.dart::TodoTest::test_todo_multiLineCommentWrapped"#,
        strict_inference: false,
        packages: &[],
        source: r#"
main() {
  /* TODO(a): Implement something
// [diag.todo][column 6][length 64] TODO(a): Implement something that is too long for one line
   *  that is too long for one line
   * This line is not part of the todo
   */
  /* TODO: Implement something
// [diag.todo][column 6][length 61] TODO: Implement something that is too long for one line
   *  that is too long for one line
   * This line is not part of the todo
   */
  /* TODO(a): Implement something
// [diag.todo][column 6][length 64] TODO(a): Implement something that is too long for one line
   *  that is too long for one line
   *
   *  This line is not part of the todo
   */
  /* TODO: Implement something
// [diag.todo][column 6][length 61] TODO: Implement something that is too long for one line
   *  that is too long for one line
   *
   *  This line is not part of the todo
  */
}
"#,
        expected: None,
    },
    Ported {
        name: r#"todo_test.dart::TodoTest::test_todo_singleLineComment"#,
        strict_inference: false,
        packages: &[],
        source: r#"
main() {
  // TODO: Implement
//   ^^^^^^^^^^^^^^^
// [diag.todo] TODO: Implement
}
"#,
        expected: None,
    },
    Ported {
        name: r#"todo_test.dart::TodoTest::test_todo_singleLineCommentFollowedByDartdoc"#,
        strict_inference: false,
        packages: &[],
        source: r#"
// TODO: Implement something
// ^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.todo] TODO: Implement something
/// This is the function documentation
void f() {}
"#,
        expected: None,
    },
    Ported {
        name: r#"todo_test.dart::TodoTest::test_todo_singleLineCommentLessIndentedContinuation"#,
        strict_inference: false,
        packages: &[],
        source: r#"
main() {
  // TODO: Implement something
// [diag.todo][column 6][length 61] TODO: Implement something that is too long for one line
  //  that is too long for one line
//    this is not part of the todo
}
"#,
        expected: None,
    },
    Ported {
        name: r#"todo_test.dart::TodoTest::test_todo_singleLineCommentMoreIndentedContinuation"#,
        strict_inference: false,
        packages: &[],
        source: r#"
main() {
  // TODO: Implement something
// [diag.todo][column 6][length 61] TODO: Implement something that is too long for one line
  //  that is too long for one line
  //      this is not part of the todo
}
"#,
        expected: None,
    },
    Ported {
        name: r#"todo_test.dart::TodoTest::test_todo_singleLineCommentNested"#,
        strict_inference: false,
        packages: &[],
        source: r#"
main() {
  // TODO: Implement something
// [diag.todo][column 6][length 61] TODO: Implement something that is too long for one line
  //  that is too long for one line
  //  TODO: This is a separate todo that is accidentally indented
//    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.todo] TODO: This is a separate todo that is accidentally indented
}
"#,
        expected: None,
    },
    Ported {
        name: r#"todo_test.dart::TodoTest::test_todo_singleLineCommentWrapped"#,
        strict_inference: false,
        packages: &[],
        source: r#"
main() {
  // TODO: Implement something
// [diag.todo][column 6][length 61] TODO: Implement something that is too long for one line
  //  that is too long for one line
  // this is not part of the todo

  // TODO: Implement something
// [diag.todo][column 6][length 61] TODO: Implement something that is too long for one line
  //  that is too long for one line

  //  this is not part of the todo

  // TODO: Implement something
// [diag.todo][column 6][length 61] TODO: Implement something that is too long for one line
  //  that is too long for one line
  //
  //  this is not part of the todo
}
"#,
        expected: None,
    },
    Ported {
        name: r#"todo_test.dart::TodoTest::test_undone"#,
        strict_inference: false,
        packages: &[],
        source: r#"
main() {
  // UNDONE: This was undone
//   ^^^^^^^^^^^^^^^^^^^^^^^
// [diag.undone] UNDONE: This was undone
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unnecessary_import_test.dart::UnnecessaryImportTest::test_library_annotationOnDirective"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
class A {
  const A() {}
}
"#,
            )],
        )],
        source: r#"
@A()
import 'lib1.dart';
"#,
        expected: None,
    },
    Ported {
        name: r#"unnecessary_import_test.dart::UnnecessaryImportTest::test_library_as_equalPrefixes_referenced"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[
                (
                    r#"lib1.dart"#,
                    r#"
class A {}
"#,
                ),
                (
                    r#"lib2.dart"#,
                    r#"
class B {}
"#,
                ),
            ],
        )],
        source: r#"
import 'lib1.dart' as one;
import 'lib2.dart' as one;
f(one.A a, one.B b) {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unnecessary_import_test.dart::UnnecessaryImportTest::test_library_as_equalPrefixes_referenced_via_export"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[
                (
                    r#"lib1.dart"#,
                    r#"
class A {}
"#,
                ),
                (
                    r#"lib2.dart"#,
                    r#"
class B {}
"#,
                ),
                (
                    r#"lib3.dart"#,
                    r#"
export 'lib2.dart';
"#,
                ),
            ],
        )],
        source: r#"
import 'lib1.dart' as one;
import 'lib3.dart' as one;
f(one.A a, one.B b) {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unnecessary_import_test.dart::UnnecessaryImportTest::test_library_as_equalPrefixes_unreferenced"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[
                (
                    r#"lib1.dart"#,
                    r#"
class A {}
"#,
                ),
                (
                    r#"lib2.dart"#,
                    r#"
class B {}
"#,
                ),
            ],
        )],
        source: r#"
import 'lib1.dart' as one;
import 'lib2.dart' as one; // ignore: unused_import
f(one.A a) {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unnecessary_import_test.dart::UnnecessaryImportTest::test_library_as_show_multipleElements"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
class A {}
class B {}
"#,
            )],
        )],
        source: r#"
import 'lib1.dart' as one show A, B;
f(one.A a, one.B b) {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unnecessary_import_test.dart::UnnecessaryImportTest::test_library_as_showTopLevelFunction"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
class One {}
topLevelFunction() {}
"#,
            )],
        )],
        source: r#"
import 'lib1.dart' hide topLevelFunction;
import 'lib1.dart' as one show topLevelFunction;
f(One o) {
  one.topLevelFunction();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unnecessary_import_test.dart::UnnecessaryImportTest::test_library_as_showTopLevelFunction_multipleDirectives"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
class One {}
topLevelFunction() {}
"#,
            )],
        )],
        source: r#"
import 'lib1.dart' hide topLevelFunction;
import 'lib1.dart' as one show topLevelFunction;
import 'lib1.dart' as two show topLevelFunction;
f(One o) {
  one.topLevelFunction();
  two.topLevelFunction();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_catch_clause_test.dart::UnusedCatchClauseTest::test_on_unusedException"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  try {
  } on String catch (exception) {
//                   ^^^^^^^^^
// [diag.unusedCatchClause] The exception variable 'exception' isn't used, so the 'catch' clause can be removed.
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_catch_clause_test.dart::UnusedCatchClauseTest::test_on_unusedStack_underscores"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  try {
  } on String catch (exception, __) {
//                              ^^
// [diag.unusedCatchStack] The stack trace variable '__' isn't used and can be removed.
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_catch_clause_test.dart::UnusedCatchClauseTest::test_on_usedException"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  try {
  } on String catch (exception) {
    print(exception);
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_catch_clause_test.dart::UnusedCatchClauseTest::test_unusedException"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  try {
  } catch (exception) {
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_catch_clause_test.dart::UnusedCatchClauseTest::test_unusedException_underscores"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  try {
  } catch (__) {
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_catch_clause_test.dart::UnusedCatchClauseTest::test_unusedException_wildcard"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  try {
  } catch (_) {
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_catch_clause_test.dart::UnusedCatchClauseTest::test_usedException"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  try {
  } catch (exception) {
    print(exception);
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_catch_stack_test.dart::UnusedCatchStackTest::test_on_unusedStack"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  try {} on String catch (exception, stackTrace) {
//                                   ^^^^^^^^^^
// [diag.unusedCatchStack] The stack trace variable 'stackTrace' isn't used and can be removed.
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_catch_stack_test.dart::UnusedCatchStackTest::test_on_usedStack"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  try {} on String catch (exception, stackTrace) {
    print(stackTrace);
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_catch_stack_test.dart::UnusedCatchStackTest::test_unusedStack"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  try {} catch (exception, stackTrace) {
//                         ^^^^^^^^^^
// [diag.unusedCatchStack] The stack trace variable 'stackTrace' isn't used and can be removed.
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_catch_stack_test.dart::UnusedCatchStackTest::test_usedStack"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  try {} catch (exception, stackTrace) {
    print(stackTrace);
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_class_field_isUsed_objectPattern"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(Object? x) {
  if (x case A(_foo: var bar)) {
    bar;
  }
}

class A {
  int _foo = 0;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_class_field_isUsed_objectPattern_generic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(Object? x) {
  if (x case A<int>(_foo: var bar)) {
    bar;
  }
}

abstract class A<T> {
  abstract T _foo;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_class_getter_isUsed_objectPattern_hasName"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(Object? x) {
  if (x case A(_foo: var bar)) {
    bar;
  }
}

class A {
  int get _foo => 0;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_class_getter_isUsed_objectPattern_hasName_generic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(Object? x) {
  if (x case A<int>(_foo: var bar)) {
    bar;
  }
}

class A<T> {
  T get _foo => throw 0;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_class_getterSetter_isUsed_assignmentExpression_compound"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  int get _foo => 0;
  set _foo(int _) {}

  void f() {
    _foo += 2;
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_class_isUsed_exposedViaTypeAlias"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {}
typedef T = _A;
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_class_isUsed_extends"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {}
class B extends _A {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_class_isUsed_fieldDeclaration"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class Foo {
  _Bar? x;
}

class _Bar {
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_class_isUsed_implements"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {}
class B implements _A {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_class_isUsed_instanceCreation"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {}
main() {
  new _A();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_class_isUsed_native"#,
        strict_inference: false,
        packages: &[],
        source: r#"
import 'dart:ffi';

final class _A extends Struct {
  @Int32() external int x;
}

final List<_A> x = [];
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_class_isUsed_staticFieldAccess"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  static const F = 42;
}
main() {
  _A.F;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_class_isUsed_staticMethodInvocation"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  static m() {}
}
main() {
  _A.m();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_class_isUsed_typeArgument"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {}
main() {
  var v = new List<_A>.empty();
  print(v);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_class_isUsed_with"#,
        strict_inference: false,
        packages: &[],
        source: r#"
mixin class _A {}
class B with _A {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_class_notUsed_inClassMember"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
//    ^^
// [diag.unusedElement] The declaration '_A' isn't referenced.
  static staticMethod() {
//       ^^^^^^^^^^^^
// [diag.unusedElement] The declaration 'staticMethod' isn't referenced.
    new _A();
  }
  instanceMethod() {
    new _A();
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_class_notUsed_inConstructorName"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
//    ^^
// [diag.unusedElement] The declaration '_A' isn't referenced.
  _A() {}
  _A.named() {}
//   ^^^^^
// [diag.unusedElement] The declaration '_A.named' isn't referenced.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_class_notUsed_isExpression"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {}
//    ^^
// [diag.unusedElement] The declaration '_A' isn't referenced.
main(p) {
  if (p is _A) {
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_class_notUsed_isExpression_typeArgument"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {}
//    ^^
// [diag.unusedElement] The declaration '_A' isn't referenced.
void f(Object p) {
  if (p is List<_A>) {
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_class_notUsed_isExpression_typeInFunctionType"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {}
//    ^^
// [diag.unusedElement] The declaration '_A' isn't referenced.
void f(Object p) {
  if (p is void Function(_A)) {
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_class_notUsed_isExpression_typeInTypeParameter"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {}
//    ^^
// [diag.unusedElement] The declaration '_A' isn't referenced.
void f(Object p) {
  if (p is void Function<T extends _A>()) {
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_class_notUsed_noReference"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {}
//    ^^
// [diag.unusedElement] The declaration '_A' isn't referenced.
main() {
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_class_setter_isUsed_assignmentExpression_simple"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  set _foo(int _) {}

  void f() {
    _foo = 0;
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_namedPrivate_fieldFormal_optionalNamed_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A._named({this.f}) {
  final int? f;
}
f() => _A._named(f: 0);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_namedPrivate_fieldFormal_optionalNamed_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A._named({this.f}) {
//                    ^
// [diag.unusedElementParameter] A value for optional parameter 'f' isn't ever given.
  final int? f;
}
f() => _A._named();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_namedPrivate_fieldFormal_optionalPositional_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A._named([this.f]) {
  final int? f;
}
f() => _A._named(0);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_namedPrivate_fieldFormal_optionalPositional_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A._named([this.f]) {
//                    ^
// [diag.unusedElementParameter] A value for optional parameter 'f' isn't ever given.
  final int? f;
}
f() => _A._named();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_namedPrivate_regularFormal_optionalNamed_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A._named({int? a});
f() => _A._named(a: 0);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_namedPrivate_regularFormal_optionalNamed_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A._named({int? a});
//                    ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
f() => _A._named();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_namedPrivate_regularFormal_optionalPositional_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A._named([int? a]);
f() => _A._named(0);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_namedPrivate_regularFormal_optionalPositional_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A._named([int? a]);
//                    ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
f() => _A._named();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_namedPrivate_superFormal_optionalNamed_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A({int? a});
class _B._named({super.a}) extends A;
var b = _B._named(a: 1);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_namedPrivate_superFormal_optionalNamed_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A({int? a});
class _B._named({super.a}) extends A;
//                     ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
var b = _B._named();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_namedPrivate_superFormal_optionalPositional_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A([int? a]);
class _B._named([super.a]) extends A;
var b = _B._named(1);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_namedPrivate_superFormal_optionalPositional_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A([int? a]);
class _B._named([super.a]) extends A;
//                     ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
var b = _B._named();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_namedPublic_fieldFormal_optionalNamed_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A.named({this.f}) {
//                   ^
// [diag.unusedElementParameter] A value for optional parameter 'f' isn't ever given.
  final int? f;
}
f() => _A.named();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_namedPublic_fieldFormal_optionalPositional_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A.named([this.f]) {
//                   ^
// [diag.unusedElementParameter] A value for optional parameter 'f' isn't ever given.
  final int? f;
}
f() => _A.named();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_namedPublic_regularFormal_optionalNamed_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A.named({int? a});
//                   ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
f() => _A.named();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_namedPublic_regularFormal_optionalPositional_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A.named([int? a]);
//                   ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
f() => _A.named();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_namedPublic_superFormal_optionalNamed_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A({int? a});
class _B.named({super.a}) extends A;
var b = _B.named(a: 1);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_namedPublic_superFormal_optionalNamed_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A({int? a});
class _B.named({super.a}) extends A;
//                    ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
var b = _B.named();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_namedPublic_superFormal_optionalPositional_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A([int? a]);
class _B.named([super.a]) extends A;
var b = _B.named(1);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_namedPublic_superFormal_optionalPositional_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A([int? a]);
class _B.named([super.a]) extends A;
//                    ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
var b = _B.named();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_unnamed_fieldFormal_optionalNamed_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A({this.f}) {
  final int? f;
}
f() => _A(f: 0);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_unnamed_fieldFormal_optionalNamed_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A({this.f}) {
//             ^
// [diag.unusedElementParameter] A value for optional parameter 'f' isn't ever given.
  final int? f;
}
f() => _A();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_unnamed_fieldFormal_optionalPositional_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A([this.f]) {
  final int? f;
}
f() => _A(0);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_unnamed_fieldFormal_optionalPositional_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A([this.f]) {
//             ^
// [diag.unusedElementParameter] A value for optional parameter 'f' isn't ever given.
  final int? f;
}
f() => _A();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_unnamed_regularFormal_optionalNamed_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A({int? a});
f() => _A(a: 0);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_unnamed_regularFormal_optionalNamed_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A({int? a});
//             ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
f() => _A();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_unnamed_regularFormal_optionalPositional_body_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A([int? a]) {}
//             ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
f() => _A();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_unnamed_regularFormal_optionalPositional_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A([int? a]);
f() => _A(0);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_unnamed_regularFormal_optionalPositional_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A([int? a]);
//             ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
f() => _A();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_unnamed_superFormal_optionalNamed_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A({int? a});
class _B({super.a}) extends A;
var b = _B(a: 1);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_unnamed_superFormal_optionalNamed_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A({int? a});
class _B({super.a}) extends A;
//              ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
var b = _B();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_unnamed_superFormal_optionalPositional_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A([int? a]);
class _B([super.a]) extends A;
var b = _B(1);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_primaryConstructor_unnamed_superFormal_optionalPositional_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A([int? a]);
class _B([super.a]) extends A;
//              ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
var b = _B();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_namedPrivate_fieldFormal_optionalNamed_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  final int? f;
  _A._named({this.f});
}
f() => _A._named(f: 0);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_namedPrivate_fieldFormal_optionalNamed_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  final int? f;
  _A._named({this.f});
//                ^
// [diag.unusedElementParameter] A value for optional parameter 'f' isn't ever given.
}
f() => _A._named();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_namedPrivate_fieldFormal_optionalPositional_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  final int? f;
  _A._named([this.f]);
}
f() => _A._named(0);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_namedPrivate_fieldFormal_optionalPositional_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  final int? f;
  _A._named([this.f]);
//                ^
// [diag.unusedElementParameter] A value for optional parameter 'f' isn't ever given.
}
f() => _A._named();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_namedPrivate_regularFormal_optionalNamed_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  _A._named({int? a});
}
f() => _A._named(a: 0);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_namedPrivate_regularFormal_optionalNamed_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  _A._named({int? a});
//                ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}
f() => _A._named();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_namedPrivate_regularFormal_optionalPositional_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  _A._named([int? a]);
}
f() => _A._named(0);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_namedPrivate_regularFormal_optionalPositional_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  _A._named([int? a]);
//                ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}
f() => _A._named();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_namedPrivate_superFormal_optionalNamed_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  _A({int? a});
}

class _B extends _A {
  _B._named({super.a});
}

var b = _B._named(a: 0);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_namedPrivate_superFormal_optionalNamed_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  _A({int? a});
}

class _B extends _A {
  _B._named({super.a});
//                 ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}

var b = _B._named();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_namedPrivate_superFormal_optionalPositional_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  _A([int? a]);
}

class _B extends _A {
  _B._named([super.a]);
}

var b = _B._named(0);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_namedPrivate_superFormal_optionalPositional_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  _A([int? a]);
}

class _B extends _A {
  _B._named([super.a]);
//                 ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}

var b = _B._named();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_namedPublic_fieldFormal_optionalNamed_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  final int? f;
  _A.named({this.f});
//               ^
// [diag.unusedElementParameter] A value for optional parameter 'f' isn't ever given.
}
f() => _A.named();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_namedPublic_fieldFormal_optionalPositional_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  final int? f;
  _A.named([this.f]);
//               ^
// [diag.unusedElementParameter] A value for optional parameter 'f' isn't ever given.
}
f() => _A.named();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_namedPublic_regularFormal_optionalNamed_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  _A.named({int? a});
//               ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}
f() => _A.named();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_namedPublic_regularFormal_optionalPositional_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  _A.named([int? a]);
//               ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}
f() => _A.named();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_unnamed_fieldFormal_optionalNamed_constructorInvocation_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  final int? f;
  _A({this.f});
}
f() => _A(f: 0);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_unnamed_fieldFormal_optionalNamed_factoryRedirect_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  final int? f;
  _A({this.f});
  factory _A.named({int? f}) = _A;
}
f() => _A.named(f: 0);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_unnamed_fieldFormal_optionalNamed_factoryRedirect_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  final int? f;
  _A({this.f});
//         ^
// [diag.unusedElementParameter] A value for optional parameter 'f' isn't ever given.
  factory _A.named() = _A;
}
f() => _A.named();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_unnamed_fieldFormal_optionalNamed_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  final int? f;
  _A({this.f});
}
f() => _A(f: 1);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_unnamed_fieldFormal_optionalNamed_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  final int? f;
  _A({this.f});
//         ^
// [diag.unusedElementParameter] A value for optional parameter 'f' isn't ever given.
}
f() => _A();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_unnamed_fieldFormal_optionalNamed_superInvocation_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  final int? e;
  _A({this.e});
}

class _B extends _A {
  _B([int? e]) : super(e: 1);
}

var b = _B(1);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_unnamed_fieldFormal_optionalNamed_superParameter_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  final int? e;
  _A({this.e});
}

class _B extends _A {
  _B({super.e});
}

var b = _B(e: 2);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_unnamed_fieldFormal_optionalPositional_constructorInvocation_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  final int? f;
  _A([this.f]);
}
f() => _A(0);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_unnamed_fieldFormal_optionalPositional_factoryRedirect_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  final int? f;
  _A([this.f]);
  factory _A.named([int? a]) = _A;
}
f() => _A.named(0);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_unnamed_fieldFormal_optionalPositional_factoryRedirect_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  final int? f;
  _A([this.f]);
//         ^
// [diag.unusedElementParameter] A value for optional parameter 'f' isn't ever given.
  factory _A.named() = _A;
}
f() => _A.named();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_unnamed_fieldFormal_optionalPositional_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  final int? f;
  _A([this.f]);
//         ^
// [diag.unusedElementParameter] A value for optional parameter 'f' isn't ever given.
}
f() => _A();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_unnamed_fieldFormal_optionalPositional_superInvocation_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  final int? e;
  _A([this.e]);
}

class _B extends _A {
  _B(int e) : super(e);
}

var b = _B(1);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_unnamed_fieldFormal_optionalPositional_superParameter_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  final int? e;
  _A([this.e]);
}

class _B extends _A {
  _B(super.e);
}

var b = _B(2);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_unnamed_regularFormal_optionalNamed_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  _A({int? a});
}
f() => _A(a: 0);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_unnamed_regularFormal_optionalNamed_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  _A({int? a});
//         ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}
f() => _A();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_unnamed_regularFormal_optionalPositional_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  _A([int a = 0]);
}
f() => _A(0);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_unnamed_regularFormal_optionalPositional_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  _A([int? a]);
//         ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}
f() => _A();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_unnamed_superFormal_optionalNamed_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  _A({int? a});
}

class _B extends _A {
  _B({super.a});
//          ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}

var b = _B();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_unnamed_superFormal_optionalPositional_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  _A([int? a]);
}

class _B extends _A {
  _B([super.a]);
}

var b = _B(1);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_unnamed_superFormal_optionalPositional_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  _A([int? a]);
}

class _B extends _A {
  _B([super.a]);
//          ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}

var b = _B();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPrivate_secondaryConstructor_unnamed_superFormal_requiredNamed_optionalNamed_overrideRequired_noDiagnostic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  A({required this.a, required this.b});
  final String a;
  final String b;
}

class _B extends A {
  _B({required super.a, super.b = 'b'});
}

var foo = _B(a: 'a');
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_primaryConstructor_namedPublic_fieldFormal_optionalNamed_noDiagnostic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A.named({this.f}) {
  final int? f;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_primaryConstructor_namedPublic_fieldFormal_optionalPositional_noDiagnostic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A.named([this.f]) {
  final int? f;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_primaryConstructor_namedPublic_regularFormal_optionalNamed_noDiagnostic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A.named({int? a});
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_primaryConstructor_namedPublic_regularFormal_optionalPositional_noDiagnostic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A.named([int? a]);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_primaryConstructor_namedPublic_superFormal_optionalNamed_noDiagnostic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A({int? a});
class B.named({super.a}) extends A;
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_primaryConstructor_namedPublic_superFormal_optionalPositional_noDiagnostic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A([int? a]);
class B.named([super.a]) extends A;
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_primaryConstructor_unnamed_fieldFormal_optionalNamed_noDiagnostic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A({this.f}) {
  final int? f;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_primaryConstructor_unnamed_fieldFormal_optionalPositional_noDiagnostic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A([this.f]) {
  final int? f;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_primaryConstructor_unnamed_regularFormal_optionalNamed_noDiagnostic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A({int? a});
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_primaryConstructor_unnamed_regularFormal_optionalPositional_noDiagnostic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A([int? a]);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_primaryConstructor_unnamed_superFormal_optionalNamed_noDiagnostic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A({int? a});
class B({super.a}) extends A;
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_primaryConstructor_unnamed_superFormal_optionalPositional_noDiagnostic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A([int? a]);
class B([super.a]) extends A;
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_secondaryConstructor_namedPrivate_fieldFormal_optionalNamed_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  final int? f;
  A._({this.f});
//          ^
// [diag.unusedElementParameter] A value for optional parameter 'f' isn't ever given.
}
f() => A._();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_secondaryConstructor_namedPrivate_fieldFormal_optionalPositional_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  final int? f;
  A._([this.f]);
//          ^
// [diag.unusedElementParameter] A value for optional parameter 'f' isn't ever given.
}
f() => A._();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_secondaryConstructor_namedPrivate_regularFormal_optionalNamed_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  A._({int? a});
}
f() => A._(a: 0);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_secondaryConstructor_namedPrivate_regularFormal_optionalNamed_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  A._({int? a});
//          ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}
f() => A._();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_secondaryConstructor_namedPrivate_regularFormal_optionalPositional_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  A._([int? a]);
}
f() => A._(0);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_secondaryConstructor_namedPrivate_regularFormal_optionalPositional_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  A._([int? a]);
//          ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}
f() => A._();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_secondaryConstructor_namedPublic_fieldFormal_optionalNamed_noDiagnostic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  final int? f;
  A.named({this.f});
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_secondaryConstructor_namedPublic_fieldFormal_optionalPositional_noDiagnostic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  final int? f;
  A.named([this.f]);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_secondaryConstructor_namedPublic_regularFormal_optionalNamed_noDiagnostic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  A.named({int? a});
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_secondaryConstructor_namedPublic_regularFormal_optionalPositional_noDiagnostic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  A.named([int? a]);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_secondaryConstructor_namedPublic_superFormal_optionalNamed_noDiagnostic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  A({int? a});
}

class B extends A {
  B.named({super.a});
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_secondaryConstructor_namedPublic_superFormal_optionalPositional_noDiagnostic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  A([int? a]);
}

class B extends A {
  B.named([super.a]);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_secondaryConstructor_unnamed_fieldFormal_optionalNamed_noDiagnostic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  final int? f;
  A({this.f});
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_secondaryConstructor_unnamed_fieldFormal_optionalPositional_noDiagnostic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  final int? f;
  A([this.f]);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_secondaryConstructor_unnamed_regularFormal_optionalNamed_noDiagnostic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  A({int? a});
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_secondaryConstructor_unnamed_regularFormal_optionalPositional_noDiagnostic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  A([int? a]);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_secondaryConstructor_unnamed_superFormal_optionalNamed_noDiagnostic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  _A({int? a});
}

class B extends _A {
  B({super.a});
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_classPublic_secondaryConstructor_unnamed_superFormal_optionalPositional_noDiagnostic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  _A([int? a]);
}

class B extends _A {
  B([super.a]);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_constructor_isUsed_asRedirectee"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  A._constructor();
  factory A.b() = A._constructor;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_constructor_isUsed_asRedirectee_viaInitializer"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  A._constructor();
  A() : this._constructor();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_constructor_isUsed_asRedirectee_viaSuper"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  A._constructor();
}

class B extends A {
  B() : super._constructor();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_constructor_isUsed_explicit"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  A._constructor();
}
A f() => A._constructor();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_constructor_isUsed_mixinApplicationRedirect"#,
        strict_inference: false,
        packages: &[],
        source: r#"
abstract class Foo {
  factory Foo({required String thing}) = _Foo._;
  Foo._({required this.thing});

  final String thing;

  void bar();
}

mixin _$Foo on Foo {
  @override
  void bar() {}
}

class _Foo = Foo with _$Foo;
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_constructor_notUsed_multiple"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  A._constructor();
//  ^^^^^^^^^^^^
// [diag.unusedElement] The declaration 'A._constructor' isn't referenced.
  A();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_constructor_notUsed_multiple_withPrimary"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A(final int i) {
  factory A._constructor() => A(7);
//          ^^^^^^^^^^^^
// [diag.unusedElement] The declaration 'A._constructor' isn't referenced.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_constructor_notUsed_single_inSubclass"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  A._constructor();
}

class B extends A {
  B() : super._constructor();
  B._named() : super._constructor();
//  ^^^^^^
// [diag.unusedElement] The declaration 'B._named' isn't referenced.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_constructorFactory_notUsed_multiple"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  factory A._factory() => A();
//          ^^^^^^^^
// [diag.unusedElement] The declaration 'A._factory' isn't referenced.
  A();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_constructorFactory_notUsed_single"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  factory A._factory() => throw 0;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_constructorPublic_privateClass_exposedViaTypeAlias"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  _A.constructor();
}
typedef T = _A;
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_constructorPublic_privateClass_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  _A.named();
//   ^^^^^
// [diag.unusedElement] The declaration '_A.named' isn't referenced.
  _A();
}
var a = _A();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_parameter_fieldFormal"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  final int? f;
  _A([this.f]);
}
void main() {
  _A a;
  a = .new(0);
  print(a);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_parameter_fieldFormal_factory"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  final int? f;
  _A([this.f]);
  factory _A.named([int? a]) = _A;
}
void main() {
  _A a;
  a = .named(0);
  print(a);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_parameter_generic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A<T> {
  _A(T a);
}
void main() {
  _A<int> a;
  a = .new(0);
  print(a);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_parameter_named"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  _A({int a = 0});
}
void main() {
  _A a;
  a = .new(a: 0);
  print(a);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_parameter_optional"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  _A([int a = 0]);
}
void main() {
  _A a;
  a = .new(0);
  print(a);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_parameter_positional"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  _A(int a);
}
void main() {
  _A a;
  a = .new(0);
  print(a);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_parameter_public_constructor"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  A([int a = 0]);
}
void main() {
  A a;
  a = .new(0);
  print(a);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_parameter_public_factory"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  final int? f;
  A([this.f]);
  factory A.named([int? a]) = A;
}
void main() {
  A a;
  a = .named(0);
  print(a);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_parameter_public_fieldFormal"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  final int? f;
  A([this.f]);
}
void main() {
  A a;
  a = .new(0);
  print(a);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_parameter_public_generic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A<T> {
  A(T a);
}
void main() {
  A<int> a;
  a = .new(0);
  print(a);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_parameter_public_named"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  A({int a = 0});
}
void main() {
  A a;
  a = .new(a: 0);
  print(a);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_parameter_public_optional"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  A([int a = 0]);
}
void main() {
  A a;
  a = .new(0);
  print(a);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_parameter_public_positional"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  A(int a);
}
void main() {
  A a;
  a = .new(0);
  print(a);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_private_constConstructorInvocation"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _C {
  const _C.named();
}

void main() {
  _C c;
  c = const .named();
  print(c);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_private_constConstructorInvocation_argument"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _C {
  const _C.named({int? p});
}
void main() {
  _C c;
  c = const .named(p: 0);
  print(c);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_private_constructorInvocation"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _C {}

void main() {
  _C c;
  c = .new();
  print(c);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_private_constructorInvocation_argument"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _C {
  _C.named({int? p});
}
void main() {
  _C c;
  c = .named(p: 0);
  print(c);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_private_enum"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E { v }

void main() {
  _E e;
  e = .v;
  print(e);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_private_extensionType"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension type _E(int i) {}

void main() {
  _E e;
  e = .new(0);
  print(e);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_private_methodInvocation"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _C {
  static _C foo() => _C();
}

void main() {
  _C c;
  c = .foo();
  print(c);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_private_methodInvocation_argument"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _C {
  static _C foo({int? p}) => _C();
}
void main() {
  _C c;
  c = .foo(p: 0);
  print(c);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_private_propertyAccess"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _C {
  static _C a = _C();
}

void main() {
  _C c;
  c = .a;
  print(c);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_public_constConstructorInvocation"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class C {
  const C.named();
}

void main() {
  C c;
  c = const .named();
  print(c);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_public_constConstructorInvocation_argument"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class C {
  const C.named({int? p});
}
void main() {
  C c;
  c = const .named(p: 0);
  print(c);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_public_constructorInvocation"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class C {}

void main() {
  C c;
  c = .new();
  print(c);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_public_constructorInvocation_argument"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class C {
  C.named({int? p});
}
void main() {
  C c;
  c = .named(p: 0);
  print(c);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_public_enum"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum E { v }

void main() {
  E e;
  e = .v;
  print(e);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_public_extensionType"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension type E(int i) {}

void main() {
  E e;
  e = .new(0);
  print(e);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_public_methodInvocation"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class C {
  static C foo() => C();
}

void main() {
  C c;
  c = .foo();
  print(c);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_public_methodInvocation_argument"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class C {
  static C foo({int? p}) => C();
}
void main() {
  C c;
  c = .foo(p: 0);
  print(c);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_dotShorthand_public_propertyAccess"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class C {
  static C a = C();
}

void main() {
  C c;
  c = .a;
  print(c);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_enum_constructor_parameter_optionalNamed_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum E {
  v(a: 0);
  const E({int? a});
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_enum_constructor_parameter_optionalNamed_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum E {
  v1, v2();
  const E({int? a});
//              ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_enum_constructor_parameter_optionalPositional_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum E {
  v(0);
  const E([int? a]);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_enum_constructor_parameter_optionalPositional_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum E {
  v1, v2();
  const E([int? a]);
//              ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_enum_isUsed_fieldReference"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _MyEnum {A}
main() {
  _MyEnum.A;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_enum_notUsed_noReference"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _MyEnum {A, B}
//   ^^^^^^^
// [diag.unusedElement] The declaration '_MyEnum' isn't referenced.
void f(d) {
  d.A;
  d.B;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_extension_unnamed_getter_isUsed_objectPattern"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(Object? x) {
  if (x case int(foo: var bar)) {
    bar;
  }
}

extension on int {
  int get foo => 0;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_extension_unnamed_getter_isUsed_objectPattern_generic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(Object? x) {
  if (x case List<int>(foo: var bar)) {
    bar;
  }
}

extension<T> on List<T> {
  T get foo => throw 0;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_extension_unnamed_operator_isUsed_generic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension<T> on T Function(T) {
  T Function(T) operator*(T Function(T) other) {
    return (value) => this(other(value));
  }
}

void f() {
  var g = (int i) => i + 1;
  g *= (i) => i + 10;
  print(g(0));
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_extension_unnamed_operator_isUsed_relationalPattern"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(int? x) {
  if (x case > 0) {}
}

extension on int? {
  bool operator >(int other) => true;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_extensionType_isUsed_typeName_typeArgument"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension type _E(int i) {}

void f() {
  Map<_E, int>();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_extensionType_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension type _E(int i) {}
//             ^^
// [diag.unusedElement] The declaration '_E' isn't referenced.
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_fieldImplicitGetter_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  int? _g;
  int? get g => this._g;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_function_underscore"#,
        strict_inference: false,
        packages: &[],
        source: r#"
_(){}
// [diag.unusedElement][column 1][length 1] The declaration '_' isn't referenced.
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_function_underscores"#,
        strict_inference: false,
        packages: &[],
        source: r#"
__(){}
// [diag.unusedElement][column 1][length 2] The declaration '__' isn't referenced.
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_functionLocal_isUsed_closure"#,
        strict_inference: false,
        packages: &[],
        source: r#"
main() {
  print(() {});
}
print(x) {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_functionLocal_isUsed_invocation"#,
        strict_inference: false,
        packages: &[],
        source: r#"
main() {
  f() {}
  f();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_functionLocal_isUsed_reference"#,
        strict_inference: false,
        packages: &[],
        source: r#"
main() {
  f() {}
  print(f);
}
print(x) {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_functionLocal_notUsed_noReference"#,
        strict_inference: false,
        packages: &[],
        source: r#"
main() {
  f() {}
//^
// [diag.unusedElement] The declaration 'f' isn't referenced.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_functionLocal_notUsed_referenceFromItself"#,
        strict_inference: false,
        packages: &[],
        source: r#"
main() {
  _f(int p) {
//^^
// [diag.unusedElement] The declaration '_f' isn't referenced.
    _f(p - 1);
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_functionTypeAlias_isUsed_isExpression"#,
        strict_inference: false,
        packages: &[],
        source: r#"
typedef _F(a, b);
main(f) {
  if (f is _F) {
    print('F');
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_functionTypeAlias_isUsed_reference"#,
        strict_inference: false,
        packages: &[],
        source: r#"
typedef _F(a, b);
void f(_F c) {
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_functionTypeAlias_isUsed_typeArgument"#,
        strict_inference: false,
        packages: &[],
        source: r#"
typedef _F(a, b);
main() {
  var v = new List<_F>.empty();
  print(v);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_functionTypeAlias_isUsed_variableDeclaration"#,
        strict_inference: false,
        packages: &[],
        source: r#"
typedef _F(a, b);
class A {
  _F? f;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_functionTypeAlias_notUsed_noReference"#,
        strict_inference: false,
        packages: &[],
        source: r#"
typedef _F(a, b);
//      ^^
// [diag.unusedElement] The declaration '_F' isn't referenced.
main() {
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_getter_isUsed_invocation_deepSubclass"#,
        strict_inference: false,
        packages: &[],
        source: r#"
abstract class A {
  String get _debugName;

  String toString() {
    return _debugName;
  }
}

class B extends A {
  @override
  String get _debugName => "B";
}

class C extends B {
  String get _debugName => "C";
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_getter_isUsed_invocation_implicitThis"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  get _g => null;
  useGetter() {
    var v = _g;
//      ^
// [diag.unusedLocalVariable] The value of the local variable 'v' isn't used.
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_getter_isUsed_invocation_parameterized"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A<T> {
  List<int> _list = List.filled(1, 1);
  int get _item => _list.first;
  set _item(int item) => _list[0] = item;
}
class B<T> {
  A<T> a = A<T>();
}
void main() {
  B<int> b = B();
  b.a._item = 3;
  print(b.a._item == 7);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_getter_isUsed_invocation_parameterized_subclass"#,
        strict_inference: false,
        packages: &[],
        source: r#"
abstract class A<T> {
  T get _defaultThing;
  T? _thing;

  void main() {
    _thing ??= _defaultThing;
    print(_thing);
  }
}
class B extends A<int> {
  @override
  int get _defaultThing => 7;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_getter_isUsed_invocation_prefixedIdentifier"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  get _g => null;
}
void f(A a) {
  var v = a._g;
//    ^
// [diag.unusedLocalVariable] The value of the local variable 'v' isn't used.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_getter_isUsed_invocation_propertyAccess"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  get _g => null;
}
main() {
  var v = new A()._g;
//    ^
// [diag.unusedLocalVariable] The value of the local variable 'v' isn't used.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_getter_isUsed_invocation_subclass_plusPlus"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  int __a = 0;
  int get _a => __a;
  void set _a(int val) {
    __a = val;
  }
  int b() => _a++;
}
class B extends A {
  @override
  int get _a => 3;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_getter_notUsed_invocation_subclass"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  int __a = 0;
  int get _a => __a;
//        ^^
// [diag.unusedElement] The declaration '_a' isn't referenced.
  void set _a(int val) {
    __a = val;
  }
  int b() => _a = 7;
}
class B extends A {
  @override
  int get _a => 3;
//        ^^
// [diag.unusedElement] The declaration '_a' isn't referenced.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_getter_notUsed_noReference"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  get _g => null;
//    ^^
// [diag.unusedElement] The declaration '_g' isn't referenced.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_getter_notUsed_referenceFromItself"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  get _g {
//    ^^
// [diag.unusedElement] The declaration '_g' isn't referenced.
    return _g;
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_localFunction_inFunction_wildcard"#,
        strict_inference: false,
        packages: &[],
        source: r#"
m() {
  _(){}
//^^^^^
// [diag.deadCode] Dead code.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_localFunction_inFunction_wildcard_preWildCards"#,
        strict_inference: false,
        packages: &[],
        source: r#"
// @dart = 3.4
// (pre wildcard-variables)

main() {
  _(){}
//^
// [diag.unusedElement] The declaration '_' isn't referenced.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_localFunction_inMethod_underscores"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class C {
  m() {
    __(){}
//  ^^
// [diag.unusedElement] The declaration '__' isn't referenced.
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_localFunction_inMethod_wildcard"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class C {
  m() {
    _(){}
//  ^^^^^
// [diag.deadCode] Dead code.
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_localFunction_inMethod_wildcard_preWildCards"#,
        strict_inference: false,
        packages: &[],
        source: r#"
// @dart = 3.4
// (pre wildcard-variables)

class C {
  m() {
    _(){}
//  ^
// [diag.unusedElement] The declaration '_' isn't referenced.
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_localFunction_underscores"#,
        strict_inference: false,
        packages: &[],
        source: r#"
main() {
  __(){}
//^^
// [diag.unusedElement] The declaration '__' isn't referenced.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_call_inExtension"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension<T> on T {
  void call() {}
}

void f() {
  (<T>(T t) => t())(7);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_hasReference_implicitThis"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  _m() {}
  useMethod() {
    print(_m);
  }
}
print(x) {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_hasReference_implicitThis_subclass"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  _m() {}
  useMethod() {
    print(_m);
  }
}
class B extends A {
  _m() {}
}
print(x) {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_hasReference_prefixedIdentifier"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  _m() {}
}
void f(A a) {
  a._m;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_hasReference_propertyAccess"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  _m() {}
}
main() {
  new A()._m;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_invocation_fromMixinApplication"#,
        strict_inference: false,
        packages: &[],
        source: r#"
mixin A {
  _m() {}
}
class C with A {
  useMethod() {
    _m();
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_invocation_fromMixinWithConstraint"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  _m() {}
}
mixin M on A {
  useMethod() {
    _m();
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_invocation_implicitThis"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  _m() {}
  useMethod() {
    _m();
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_invocation_implicitThis_subclass"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  _m() {}
  useMethod() {
    _m();
  }
}
class B extends A {
  _m() {}
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_invocation_memberElement"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A<T> {
  _m(T t) {}
}
void f(A<int> a) {
  a._m(0);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_invocation_propagated"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  _m() {}
}
main() {
  var a = new A();
  a._m();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_invocation_static"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  _m() {}
}
main() {
  A a = new A();
  a._m();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_invocation_subclass"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  _m() {}
}
class B extends A {
  _m() {}
}
void f(A a) {
  a._m();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_privateExtension"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension _A on String {
  void m() {}
}
void main() {
  "hello".m();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_privateExtension_binaryOperator"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension _A on String {
  int operator -(int other) => other;
}
void main() {
  "hello" - 3;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_privateExtension_generic_binaryOperator"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A<T> {}
extension _A<T> on A<T> {
  int operator -(int other) => other;
}
void f(A<int> a) {
  a - 3;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_privateExtension_generic_indexEqOperator"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A<T> {}
extension _A<T> on A<T> {
  void operator []=(int index, T value) {
}}
void f(A<int> a) {
  a[0] = 1;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_privateExtension_generic_indexOperator"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A<T> {}
extension _A<T> on A<T> {
  A<T> operator [](int index) => throw 0;
}
void f(A<int> a) {
  a[0];
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_privateExtension_generic_method"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A<T> {}
extension _A<T> on A<T> {
  A<T> foo() => throw 0;
}
void f(A<int> a) {
  a.foo();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_privateExtension_generic_postfixOperator"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A<T> {}
extension _A<T> on A<T> {
  A<T> operator -(int i) => throw 0;
}
void f(A<int> a) {
  a--;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_privateExtension_generic_prefixOperator"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A<T> {}
extension _A<T> on A<T> {
  T operator ~() => throw 0;
}
void f(A<int> a) {
  ~a;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_privateExtension_indexEqOperator"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension _A on bool {
  operator []=(int index, int value) {}
}
void main() {
  false[0] = 1;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_privateExtension_indexOperator"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension _A on bool {
  int operator [](int index) => 7;
}
void main() {
  false[3];
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_privateExtension_methodCall"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension _E on int {
  void call() {}
}

void f() {
  0();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_privateExtension_operator_assignment"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension _A on String {
  String operator -(int other) => this;
}
void f(String s) {
  s -= 3;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_privateExtension_postfixOperator"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension _A on String {
  String operator -(int i) => this;
}
void f(String a) {
  a--;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_privateExtension_prefixOperator"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension _A on String {
  int operator ~() => 7;
}
void main() {
  ~"hello";
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_public"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  m() {}
}
main() {
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_staticInvocation"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  static _m() {}
}
main() {
  A._m();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_unnamedExtension"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension on String {
  void m() {}
}
void main() {
  "hello".m();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_unnamedExtension_methodCall"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension on int {
  void call() {}
}

void f() {
  0();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_isUsed_unnamedExtension_operator"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension on String {
  int operator -(int other) => other;
}
void main() {
  "hello" - 3;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_notUsed_call_inExtension"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension<T> on T {
  void call() {}
//     ^^^^
// [diag.unusedElement] The declaration 'call' isn't referenced.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_notUsed_hasSameNameAsUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  void _m1() {}
//     ^^^
// [diag.unusedElement] The declaration '_m1' isn't referenced.
}
class B {
  void public() => _m1();
  void _m1() {}
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_notUsed_noReference"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  static _m() {}
//       ^^
// [diag.unusedElement] The declaration '_m' isn't referenced.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_notUsed_privateExtension_indexEqOperator"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension _A on bool {
  operator []=(int index, int value) {}
//         ^^^
// [diag.unusedElement] The declaration '[]=' isn't referenced.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_notUsed_privateExtension_indexOperator"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension _A on bool {
  int operator [](int index) => 7;
//             ^^
// [diag.unusedElement] The declaration '[]' isn't referenced.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_notUsed_privateExtension_operator"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension _A on String {
  int operator -(int other) => other;
//             ^
// [diag.unusedElement] The declaration '-' isn't referenced.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_notUsed_privateExtension_prefixOperator"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension _A on String {
  int operator ~() => 7;
//             ^
// [diag.unusedElement] The declaration '~' isn't referenced.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_notUsed_referenceFromItself"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  static _m(int p) {
//       ^^
// [diag.unusedElement] The declaration '_m' isn't referenced.
    _m(p - 1);
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_notUsed_referenceInComment"#,
        strict_inference: false,
        packages: &[],
        source: r#"
/// [A] has a method, [_f].
class A {
  int _f(int p) => 7;
//    ^^
// [diag.unusedElement] The declaration '_f' isn't referenced.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_notUsed_referenceInComment_outsideEnclosingClass"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  int _f(int p) => 7;
//    ^^
// [diag.unusedElement] The declaration '_f' isn't referenced.
}
/// This is similar to [A._f].
int g() => 7;
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_notUsed_unnamedExtension"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension on String {
  void m() {}
//     ^
// [diag.unusedElement] The declaration 'm' isn't referenced.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_method_notUsed_unnamedExtension_operator"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension on String {
  int operator -(int other) => other;
//             ^
// [diag.unusedElement] The declaration '-' isn't referenced.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_mixin_isUsed_with"#,
        strict_inference: false,
        packages: &[],
        source: r#"
mixin _M {}
class C with _M {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_mixin_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
mixin _M {}
//    ^^
// [diag.unusedElement] The declaration '_M' isn't referenced.
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_parameter_isUsed_functionTearoff"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  void _m([int? a]) {}
  _m;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_parameter_isUsed_inAnnotation"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _MyAnnotation {
  const _MyAnnotation({this.value});
  final int? value;
}

@_MyAnnotation(value: 42)
void fn() {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_parameter_isUsed_local"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  void _m([int? a]) {}
  _m(1);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_parameter_isUsed_methodTearoff"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  void _m([int? a]) {}
}
f() => A()._m;
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_parameter_isUsed_named"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  void _m({int a = 0}) {}
}
f() => A()._m(a: 0);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_parameter_isUsed_overridden"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  void _m([int? a]) {}
//              ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}
class B implements A {
  void _m([int? a]) {}
}
f() {
  A()._m();
  B()._m(0);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_parameter_isUsed_override"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  void _m([int? a]) {}
}
class B implements A {
  void _m([int? a]) {}
}
f() => A()._m(0);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_parameter_isUsed_override_renamed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  void _m([int? a]) {}
}
class B implements A {
  void _m([int? b]) {}
}
f() => A()._m(0);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_parameter_isUsed_overrideRequired"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  void _m(int a) {}
}
class B implements A {
  void _m([int? a]) {}
}
f() => A()._m(0);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_parameter_isUsed_overrideRequiredNamed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  void _m({required int a}) {}
}
class B implements A {
  void _m({int a = 0}) {}
}
f() => A()._m(a: 0);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_parameter_isUsed_positional"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  void _m([int? a]) {}
}
f() => A()._m(0);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_parameter_isUsed_publicMethod"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  void m([int? a]) {}
}
f() => A().m();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_parameter_isUsed_publicMethod_extension"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension E on String {
  void m([int? a]) {}
}
f() => "hello".m();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_parameter_isUsed_requiredPositional"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  void _m(int a) {}
}
f() => A()._m(0);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_parameter_isUsed_superParameter_inPrimaryConstructor_optionalNamed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _BaseNamedOptional({final int? value});

class SubNamedOptional({super.value}) extends _BaseNamedOptional;

void main() {
  print(SubNamedOptional(value: 42));
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_parameter_isUsed_superParameter_inPrimaryConstructor_optionalPositional"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _BaseOptional([final int? value]);

class SubOptional(super.value) extends _BaseOptional;

void main() {
  print(SubOptional(42));
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_parameter_isUsed_topLevel"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void _m([int? a]) {}
f() => _m(1);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_parameter_isUsed_topLevelPublic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void m([int? a]) {}
f() => m();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_parameter_missingName_isNamed_redirectingFactory_source"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class C {
  C.impl({int? x});
  factory C({}) = C.impl;
//           ^
// [diag.missingIdentifier] Expected an identifier.
//                ^^^^^^
// [diag.redirectToInvalidFunctionType] The redirected constructor 'C Function({int? x})' has incompatible parameters with 'C Function({dynamic})'.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_parameter_missingName_isNamed_redirectingFactory_target"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class C {
  C.impl({});
//        ^
// [diag.missingIdentifier] Expected an identifier.
  factory C({int? x}) = C.impl;
//                      ^^^^^^
// [diag.redirectToInvalidFunctionType] The redirected constructor 'C Function({dynamic})' has incompatible parameters with 'C Function({int? x})'.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_parameter_notUsed_extension"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension E on String {
  void _m([int? a]) {}
//              ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}
f() => "hello"._m();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_parameter_notUsed_named"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  void _m({int? a}) {}
//              ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}
f() => A()._m();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_parameter_notUsed_override_added"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  void _m() {}
}
class B implements A {
  void _m([int? a]) {}
//              ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}
f() => A()._m();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_parameter_notUsed_positional"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  void _m([int? a]) {}
//              ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}
f() => A()._m();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_parameter_notUsed_publicMethod_privateExtension"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension _E on String {
  void m([int? a]) {}
//             ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}
f() => "hello".m();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_parameter_notUsed_publicMethod_unnamedExtension"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension on String {
  void m([int? a]) {}
//             ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}
f() => "hello".m();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_parameter_notUsed_static"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  static void _m([int? a]) {}
//                     ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}
f() => A._m();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_parameter_notUsed_staticPublic_privateClass"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  static void m([int? a]) {}
//                    ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}
f() => _A.m();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_parameter_notUsed_topLevel"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void _m([int? a]) {}
//            ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
f() => _m();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_privateConstructor_isUsed_redirect"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v._foo();
  const _E._foo() : this._bar();
  const _E._bar();
}

void f() {
  _E.v;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_privateConstructor_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v._foo();
  const _E._foo();
  const _E._bar();
//         ^^^^
// [diag.unusedElement] The declaration '_E._bar' isn't referenced.
}

void f() {
  _E.v;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_privateInstanceGetter_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  int get _foo => 0;
}

void f() {
  _E.v._foo;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_privateInstanceGetter_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  int get _foo => 0;
//        ^^^^
// [diag.unusedElement] The declaration '_foo' isn't referenced.
}

void f() {
  _E.v;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_privateInstanceMethod_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  void _foo() {}
}

void f() {
  _E.v._foo();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_privateInstanceMethod_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  void _foo() {}
//     ^^^^
// [diag.unusedElement] The declaration '_foo' isn't referenced.
}

void f() {
  _E.v;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_privateInstanceMethod_optionalNamedParameter_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  void _foo({int? a}) {}
}

void f() {
  _E.v._foo(a: 0);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_privateInstanceMethod_optionalNamedParameter_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  void _foo({int? a}) {}
//                ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}

void f() {
  _E.v._foo();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_privateInstanceMethod_optionalPositionalParameter_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  void _foo([int? a]) {}
}

void f() {
  _E.v._foo(0);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_privateInstanceMethod_optionalPositionalParameter_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  void _foo([int? a]) {}
//                ^
// [diag.unusedElementParameter] A value for optional parameter 'a' isn't ever given.
}

void f() {
  _E.v._foo();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_privateInstanceSetter_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  set _foo(int _) {}
}

void f() {
  _E.v._foo = 0;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_privateInstanceSetter_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  set _foo(int _) {}
//    ^^^^
// [diag.unusedElement] The declaration '_foo' isn't referenced.
}

void f() {
  _E.v;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_privateStaticGetter_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  static int get _foo => 0;
}

void f() {
  _E.v;
  _E._foo;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_privateStaticGetter_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  static int get _foo => 0;
//               ^^^^
// [diag.unusedElement] The declaration '_foo' isn't referenced.
}

void f() {
  _E.v;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_privateStaticMethod_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  static void _foo() {}
}

void f() {
  _E.v;
  _E._foo();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_privateStaticMethod_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  static void _foo() {}
//            ^^^^
// [diag.unusedElement] The declaration '_foo' isn't referenced.
}

void f() {
  _E.v;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_privateStaticSetter_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  static set _foo(int _) {}
}

void f() {
  _E.v;
  _E._foo = 0;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_privateStaticSetter_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  static set _foo(int _) {}
//           ^^^^
// [diag.unusedElement] The declaration '_foo' isn't referenced.
}

void f() {
  _E.v;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_publicConstructor_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v.foo();
  const _E.foo();
  const _E.bar();
//         ^^^
// [diag.unusedElement] The declaration '_E.bar' isn't referenced.
}

void f() {
  _E.v;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_publicInstanceGetter_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  int get foo => 0;
}

void f() {
  _E.v;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_publicInstanceMethod_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  void foo() {}
}

void f() {
  _E.v;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_publicInstanceMethod_optionalNamedParameter_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  void foo({int? a}) {}
}

void f() {
  _E.v.foo();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_publicInstanceMethod_optionalPositionalParameter_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  void foo([int? a]) {}
}

void f() {
  _E.v.foo();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_publicInstanceSetter_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  set foo(int _) {}
}

void f() {
  _E.v;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_publicStaticGetter_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  static int get foo => 0;
}

void f() {
  _E.v;
  _E.foo;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_publicStaticGetter_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  static int get foo => 0;
//               ^^^
// [diag.unusedElement] The declaration 'foo' isn't referenced.
}

void f() {
  _E.v;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_publicStaticMethod_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  static void foo() {}
}

void f() {
  _E.v;
  _E.foo();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_publicStaticMethod_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  static void foo() {}
//            ^^^
// [diag.unusedElement] The declaration 'foo' isn't referenced.
}

void f() {
  _E.v;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_publicStaticSetter_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  static set foo(int _) {}
}

void f() {
  _E.v;
  _E.foo = 0;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_privateEnum_publicStaticSetter_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  static set foo(int _) {}
//           ^^^
// [diag.unusedElement] The declaration 'foo' isn't referenced.
}

void f() {
  _E.v;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_publicEnum_privateConstructor_isUsed_redirect"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum E {
  v._foo();
  const E._foo() : this._bar();
  const E._bar();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_publicEnum_privateConstructor_notExposedViaTypeAlias"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  one(), two();
  const _E();
  const _E.named();
}
typedef T = _E;
void f() {
  _E.one;
  _E.two;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_publicEnum_privateConstructor_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum E {
  v._foo();
  const E._foo();
  const E._bar();
//        ^^^^
// [diag.unusedElement] The declaration 'E._bar' isn't referenced.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_publicEnum_privateStaticGetter_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum E {
  v;
  static int get _foo => 0;
}

void f() {
  E._foo;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_publicEnum_privateStaticGetter_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum E {
  v;
  static int get _foo => 0;
//               ^^^^
// [diag.unusedElement] The declaration '_foo' isn't referenced.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_publicEnum_privateStaticMethod_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum E {
  v;
  static void _foo() {}
}

void f() {
  E._foo();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_publicEnum_privateStaticMethod_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum E {
  v;
  static void _foo() {}
//            ^^^^
// [diag.unusedElement] The declaration '_foo' isn't referenced.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_publicEnum_privateStaticSetter_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum E {
  v;
  static set _foo(int _) {}
}

void f() {
  E._foo = 0;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_publicEnum_privateStaticSetter_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum E {
  v;
  static set _foo(int _) {}
//           ^^^^
// [diag.unusedElement] The declaration '_foo' isn't referenced.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_publicEnum_publicConstructor_isUsed_generic"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum E<T> {
  v1<int>.named(),
  v2<int>.renamed();

  const E.named();
  const E.renamed() : this.named();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_publicEnum_publicConstructor_isUsed_redirect"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum E {
  v.foo();
  const E.foo() : this.bar();
  const E.bar();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_publicEnum_publicConstructor_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum E {
  v.foo();
  const E.foo();
  const E.bar();
//        ^^^
// [diag.unusedElement] The declaration 'E.bar' isn't referenced.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_publicEnum_publicStaticGetter_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum E {
  v;
  static int get foo => 0;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_publicEnum_publicStaticMethod_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum E {
  v;
  static void foo() {}
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_publicEnum_publicStaticSetter_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum E {
  v;
  static set foo(int _) {}
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_publicStaticMethod_privateClass_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  static void m() {}
}
void main() {
  _A.m();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_publicStaticMethod_privateClass_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  static void m() {}
//            ^
// [diag.unusedElement] The declaration 'm' isn't referenced.
}
void f(_A a) {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_publicStaticMethod_privateExtension_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension _A on String {
  static void m() {}
}
void main() {
  _A.m();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_publicStaticMethod_privateExtension_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension _A on String {
  static void m() {}
//            ^
// [diag.unusedElement] The declaration 'm' isn't referenced.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_publicStaticMethod_privateMixin_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
mixin _A {
  static void m() {}
}
void main() {
  _A.m();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_publicStaticMethod_privateMixin_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
mixin _A {
  static void m() {}
//            ^
// [diag.unusedElement] The declaration 'm' isn't referenced.
}
void main() {
  _A;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_publicTopLevelFunction_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
int get a => 1;
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_setter_isUsed_invocation_implicitThis"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  set _s(x) {}
  useSetter() {
    _s = 42;
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_setter_isUsed_invocation_PrefixedIdentifier"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  set _s(x) {}
}
void f(A a) {
  a._s = 42;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_setter_isUsed_invocation_PropertyAccess"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  set _s(x) {}
}
main() {
  new A()._s = 42;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_setter_isUsed_subclass_viaExtension"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  set _value(int v) {}
}

extension E on A {
  set value(int v) => _value = v;
}

class B extends A {
  @override
  set _value(int v) {}
}


void main() {
  A().value = 1;
  B().value = 1;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_setter_isUsed_topLevelFunction"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  set _value(int v) {}
}

void f() {
  A()._value = 1;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_setter_notUsed_noReference"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  set _s(x) {}
//    ^^
// [diag.unusedElement] The declaration '_s' isn't referenced.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_setter_notUsed_referenceFromItself"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  set _s(int x) {
//    ^^
// [diag.unusedElement] The declaration '_s' isn't referenced.
    if (x > 5) {
      _s = x - 1;
    }
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_topLevelAccessors_isUsed_questionQuestionEqual"#,
        strict_inference: false,
        packages: &[],
        source: r#"
int? get _c => 1;
void set _c(int? x) {}
int f() {
  return _c ??= 7;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_topLevelFunction_isUsed_hasPragma_vmEntryPoint"#,
        strict_inference: false,
        packages: &[],
        source: r#"
@pragma('vm:entry-point')
void _f() {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_topLevelFunction_isUsed_invocation"#,
        strict_inference: false,
        packages: &[],
        source: r#"
_f() {}
main() {
  _f();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_topLevelFunction_isUsed_reference"#,
        strict_inference: false,
        packages: &[],
        source: r#"
_f() {}
main() {
  print(_f);
}
print(x) {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_topLevelFunction_notUsed_noReference"#,
        strict_inference: false,
        packages: &[],
        source: r#"
_f() {}
// [diag.unusedElement][column 1][length 2] The declaration '_f' isn't referenced.
main() {
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_topLevelFunction_notUsed_referenceFromItself"#,
        strict_inference: false,
        packages: &[],
        source: r#"
_f(int p) {
// [diag.unusedElement][column 1][length 2] The declaration '_f' isn't referenced.
  _f(p - 1);
}
main() {
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_topLevelFunction_notUsed_referenceInComment"#,
        strict_inference: false,
        packages: &[],
        source: r#"
/// [_f] is a great function.
_f(int p) => 7;
// [diag.unusedElement][column 1][length 2] The declaration '_f' isn't referenced.
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_topLevelGetterSetter_isUsed_assignmentExpression_compound"#,
        strict_inference: false,
        packages: &[],
        source: r#"
int get _foo => 0;
set _foo(int _) {}

void f() {
  _foo += 2;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_topLevelGetterSetter_isUsed_postfixExpression_increment"#,
        strict_inference: false,
        packages: &[],
        source: r#"
int get _foo => 0;
set _foo(int _) {}

void f() {
  _foo++;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_topLevelGetterSetter_isUsed_prefixExpression_increment"#,
        strict_inference: false,
        packages: &[],
        source: r#"
int get _foo => 0;
set _foo(int _) {}

void f() {
  ++_foo;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_topLevelSetter_isUsed_assignmentExpression_simple"#,
        strict_inference: false,
        packages: &[],
        source: r#"
set _foo(int _) {}

void f() {
  _foo = 0;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_topLevelSetter_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
set _foo(int _) {}
//  ^^^^
// [diag.unusedElement] The declaration '_foo' isn't referenced.
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_topLevelVariable_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
int _a = 1;
main() {
  _a;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_topLevelVariable_isUsed_plusPlus"#,
        strict_inference: false,
        packages: &[],
        source: r#"
int _a = 0;
main() {
  var b = _a++;
  b;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_topLevelVariable_isUsed_questionQuestionEqual"#,
        strict_inference: false,
        packages: &[],
        source: r#"
int? _a;
f() {
  _a ??= 1;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_topLevelVariable_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
int _a = 1;
//  ^^
// [diag.unusedElement] The declaration '_a' isn't referenced.
main() {
  _a = 2;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_topLevelVariable_notUsed_compoundAssign"#,
        strict_inference: false,
        packages: &[],
        source: r#"
int _a = 1;
//  ^^
// [diag.unusedElement] The declaration '_a' isn't referenced.
f() {
  _a += 1;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_topLevelVariable_notUsed_referenceInComment"#,
        strict_inference: false,
        packages: &[],
        source: r#"
/// [_a] is a great variable.
int _a = 7;
//  ^^
// [diag.unusedElement] The declaration '_a' isn't referenced.
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_typeAlias_functionType_isUsed_isExpression"#,
        strict_inference: false,
        packages: &[],
        source: r#"
typedef _F = void Function();
main(f) {
  if (f is _F) {
    print('F');
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_typeAlias_functionType_isUsed_reference"#,
        strict_inference: false,
        packages: &[],
        source: r#"
typedef _F = void Function();
void f(_F f) {
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_typeAlias_functionType_isUsed_typeArgument"#,
        strict_inference: false,
        packages: &[],
        source: r#"
typedef _F = void Function();
main() {
  var v = new List<_F>.empty();
  print(v);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_typeAlias_functionType_isUsed_variableDeclaration"#,
        strict_inference: false,
        packages: &[],
        source: r#"
typedef _F = void Function();
class A {
  _F? f;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_typeAlias_functionType_notUsed_noReference"#,
        strict_inference: false,
        packages: &[],
        source: r#"
typedef _F = void Function();
//      ^^
// [diag.unusedElement] The declaration '_F' isn't referenced.
main() {
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_typeAlias_interfaceType_isUsed_typeName_isExpression"#,
        strict_inference: false,
        packages: &[],
        source: r#"
typedef _A = List<int>;

void f(a) {
  a is _A;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_typeAlias_interfaceType_isUsed_typeName_parameter"#,
        strict_inference: false,
        packages: &[],
        source: r#"
typedef _A = List<int>;

void f(_A a) {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_typeAlias_interfaceType_isUsed_typeName_typeArgument"#,
        strict_inference: false,
        packages: &[],
        source: r#"
typedef _A = List<int>;

void f() {
  Map<_A, int>();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_element_test.dart::UnusedElementTest::test_typeAlias_interfaceType_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
typedef _A = List<int>;
//      ^^
// [diag.unusedElement] The declaration '_A' isn't referenced.
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_from_primary_constructor_test.dart::UnusedFieldFromPrimaryConstructorTest::test_isUsed_class_declaringFormal_requiredPositional"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A(final int _i) {
  int get x => _i;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_from_primary_constructor_test.dart::UnusedFieldFromPrimaryConstructorTest::test_isUsed_class_declaringFormal_requiredPositional_public"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A(final int i) {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_from_primary_constructor_test.dart::UnusedFieldFromPrimaryConstructorTest::test_isUsed_class_fieldFormal"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A(this._f) {
  int _f;
  int get x => _f;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_from_primary_constructor_test.dart::UnusedFieldFromPrimaryConstructorTest::test_isUsed_extensionType_declaringFormal_requiredPositional_underscore"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension type A(final int _) {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_from_primary_constructor_test.dart::UnusedFieldFromPrimaryConstructorTest::test_notUsed_class_declaringFormal_optionalNamed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A({final int _i = 0}) {}
//                 ^^
// [diag.unusedFieldFromPrimaryConstructor] The value of the field '_i' isn't used.
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_from_primary_constructor_test.dart::UnusedFieldFromPrimaryConstructorTest::test_notUsed_class_declaringFormal_optionalNamed_functionTyped"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A({final void _f() = _g}) {}
//                  ^^
// [diag.unusedFieldFromPrimaryConstructor] The value of the field '_f' isn't used.
void _g() {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_from_primary_constructor_test.dart::UnusedFieldFromPrimaryConstructorTest::test_notUsed_class_declaringFormal_requiredPositional"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A(final int _i) {}
//                ^^
// [diag.unusedFieldFromPrimaryConstructor] The value of the field '_i' isn't used.
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_from_primary_constructor_test.dart::UnusedFieldFromPrimaryConstructorTest::test_notUsed_class_declaringFormal_requiredPositional_functionTyped"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A(final int _f()) {}
//                ^^
// [diag.unusedFieldFromPrimaryConstructor] The value of the field '_f' isn't used.
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_from_primary_constructor_test.dart::UnusedFieldFromPrimaryConstructorTest::test_notUsed_class_declaringFormal_requiredPositional_underscore"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A(final int _) {}
//                ^
// [diag.unusedFieldFromPrimaryConstructor] The value of the field '_' isn't used.
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_from_primary_constructor_test.dart::UnusedFieldFromPrimaryConstructorTest::test_notUsed_class_fieldFormal"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A(this._f) {
  int _f;
//    ^^
// [diag.unusedField] The value of the field '_f' isn't used.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_isUsed_argument"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  int _f = 0;
  main() {
    print(++_f);
  }
}
print(x) {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_isUsed_extensionOnClass"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class Foo {}
extension Bar on Foo {
  int baz() => _baz;
  static final _baz = 7;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_isUsed_extensionOnEnum"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum Foo {a, b}
extension Bar on Foo {
  int baz() => _baz;
  static final _baz = 1;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_isUsed_mixin"#,
        strict_inference: false,
        packages: &[],
        source: r#"
mixin M {
  int _f = 0;
}
class Bar with M {
  int g() => _f;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_isUsed_mixinRestriction"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class Foo {
  int _f = 0;
}
mixin M on Foo {
  int g() => _f;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_isUsed_parameterized_subclass"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A<T extends num> {
  T _f;
  A._(this._f);
}
class B extends A<int> {
  B._(int f) : super._(f);
}
void main() {
  B b = B._(7);
  print(b._f == 7);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_isUsed_publicStaticField_privateClass"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  static String f1 = "x";
}
void main() => print(_A.f1);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_isUsed_publicStaticField_privateExtension"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension _A on String {
  static String f1 = "x";
}
void main() => print(_A.f1);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_isUsed_publicStaticField_privateMixin"#,
        strict_inference: false,
        packages: &[],
        source: r#"
mixin _A {
  static String f1 = "x";
}
void main() => print(_A.f1);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_isUsed_reference_implicitThis"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  int _f = 0;
  main() {
    print(_f);
  }
}
print(x) {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_isUsed_reference_implicitThis_expressionFunctionBody"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  int _f = 0;
  m() => _f;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_isUsed_reference_implicitThis_subclass"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  int _f = 0;
  main() {
    print(_f);
  }
}
class B extends A {
  int _f = 0;
}
print(x) {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_isUsed_reference_qualified_propagatedElement"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  int _f = 0;
}
main() {
  var a = new A();
  print(a._f);
}
print(x) {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_isUsed_reference_qualified_staticElement"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  int _f = 0;
}
main() {
  A a = new A();
  print(a._f);
}
print(x) {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_isUsed_reference_qualified_unresolved"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  int _f = 0;
}
main(a) {
  print(a._f);
}
print(x) {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_isUsed_underscoreField_shadowsParameter"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  var _ = 1;
  void m(int? _) {
    print(_);
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_notUsed_compoundAssign"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  int _f = 0;
//    ^^
// [diag.unusedField] The value of the field '_f' isn't used.
  main() {
    _f += 2;
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_notUsed_constructorFieldInitializers"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  int _f;
//    ^^
// [diag.unusedField] The value of the field '_f' isn't used.
  A() : _f = 0;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_notUsed_extensionOnClass"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class Foo {}
extension Bar on Foo {
  static final _baz = 7;
//             ^^^^
// [diag.unusedField] The value of the field '_baz' isn't used.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_notUsed_fieldFormalParameter"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  int _f;
//    ^^
// [diag.unusedField] The value of the field '_f' isn't used.
  A(this._f);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_notUsed_mixin"#,
        strict_inference: false,
        packages: &[],
        source: r#"
mixin M {
  int _f = 0;
//    ^^
// [diag.unusedField] The value of the field '_f' isn't used.
}
class Bar with M {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_notUsed_mixinRestriction"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class Foo {
  int _f = 0;
//    ^^
// [diag.unusedField] The value of the field '_f' isn't used.
}
mixin M on Foo {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_notUsed_noReference"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  int _f = 0;
//    ^^
// [diag.unusedField] The value of the field '_f' isn't used.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_notUsed_noReference_wildcard"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  int _ = 0;
//    ^
// [diag.unusedField] The value of the field '_' isn't used.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_notUsed_noReference_wildcard_preWildcards"#,
        strict_inference: false,
        packages: &[],
        source: r#"
// @dart = 3.4
// (pre wildcard-variables)

class A {
  int _ = 0;
//    ^
// [diag.unusedField] The value of the field '_' isn't used.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_notUsed_nullAssign"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  var _f;
  m() {
    _f ??= doSomething();
  }
}
doSomething() => 0;
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_notUsed_postfixExpr"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  int _f = 0;
//    ^^
// [diag.unusedField] The value of the field '_f' isn't used.
  main() {
    _f++;
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_notUsed_prefixExpr"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  int _f = 0;
//    ^^
// [diag.unusedField] The value of the field '_f' isn't used.
  main() {
    ++_f;
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_notUsed_publicStaticField_privateClass"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class _A {
  static String f1 = "x";
//              ^^
// [diag.unusedField] The value of the field 'f1' isn't used.
}
void main() => print(_A);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_notUsed_publicStaticField_privateExtension"#,
        strict_inference: false,
        packages: &[],
        source: r#"
extension _A on String {
  static String f1 = "x";
//              ^^
// [diag.unusedField] The value of the field 'f1' isn't used.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_notUsed_publicStaticField_privateMixin"#,
        strict_inference: false,
        packages: &[],
        source: r#"
mixin _A {
  static String f1 = "x";
//              ^^
// [diag.unusedField] The value of the field 'f1' isn't used.
}
void main() => print(_A);
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_notUsed_referenceInComment"#,
        strict_inference: false,
        packages: &[],
        source: r#"
/// [A._f] is great.
class A {
  int _f = 0;
//    ^^
// [diag.unusedField] The value of the field '_f' isn't used.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_notUsed_simpleAssignment"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  int _f = 0;
//    ^^
// [diag.unusedField] The value of the field '_f' isn't used.
  m() {
    _f = 1;
  }
}
f(A a) {
  a._f = 2;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_privateEnum_publicConstant_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
}

void f() {
 _E.v;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_privateEnum_publicConstant_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
//^
// [diag.unusedField] The value of the field 'v' isn't used.
}

void f() {
  _E;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_privateEnum_publicInstanceField_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  final int foo = 0;
}

void f() {
  _E.v;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_privateEnum_publicStaticField_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  static final int foo = 0;
}

void f() {
  _E.v;
  _E.foo;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_privateEnum_publicStaticField_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  static final int foo = 0;
//                 ^^^
// [diag.unusedField] The value of the field 'foo' isn't used.
}

void f() {
  _E.v;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_privateEnum_values_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v
}

void f() {
  _E.values;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_privateEnum_values_isUsed_hasSetter"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum _E {
  v;
  set foo(int _) {}
}

void f() {
  _E.values;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_publicEnum_privateConstant_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum E {
  _v
}

void f() {
  E._v;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_publicEnum_privateConstant_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum E {
  _v
//^^
// [diag.unusedField] The value of the field '_v' isn't used.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_publicEnum_privateInstanceField_isUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum E {
  v;
  final int _foo = 0;
}

void f() {
  E.v._foo;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_field_test.dart::UnusedFieldTest::test_publicEnum_privateInstanceField_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
enum E {
  v;
  final int _foo = 0;
//          ^^^^
// [diag.unusedField] The value of the field '_foo' isn't used.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_annotationOnDirective"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
class A {
  const A() {}
}
"#,
            )],
        )],
        source: r#"
@A()
import 'lib1.dart';
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_core_library"#,
        strict_inference: false,
        packages: &[],
        source: r#"
import 'dart:core';
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_export"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[
                (
                    r#"lib1.dart"#,
                    r#"
export 'lib2.dart';
class One {}
"#,
                ),
                (
                    r#"lib2.dart"#,
                    r#"
class Two {}
"#,
                ),
            ],
        )],
        source: r#"
import 'lib1.dart';
Two two = Two();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_export2"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[
                (
                    r#"lib1.dart"#,
                    r#"
export 'lib2.dart';
class One {}
"#,
                ),
                (
                    r#"lib2.dart"#,
                    r#"
export 'lib3.dart';
class Two {}
"#,
                ),
                (
                    r#"lib3.dart"#,
                    r#"
class Three {}
"#,
                ),
            ],
        )],
        source: r#"
import 'lib1.dart';
Three? three;
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_export_infiniteLoop"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[
                (
                    r#"lib1.dart"#,
                    r#"
export 'lib2.dart';
class One {}
"#,
                ),
                (
                    r#"lib2.dart"#,
                    r#"
export 'lib3.dart';
class Two {}
"#,
                ),
                (
                    r#"lib3.dart"#,
                    r#"
export 'lib2.dart';
class Three {}
"#,
                ),
            ],
        )],
        source: r#"
import 'lib1.dart';
Two? two;
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_extension_instance_call"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
extension E on int {
  int call(int x) => 0;
}
"#,
            )],
        )],
        source: r#"
import 'lib1.dart';

f() {
  7(9);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_extension_instance_getter"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
extension E on String {
  String get empty => '';
}
"#,
            )],
        )],
        source: r#"
import 'lib1.dart';

f() {
  ''.empty;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_extension_instance_getter_fromObjectPattern"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"a.dart"#,
                r#"
extension E on int {
  bool get foo => true;
}
"#,
            )],
        )],
        source: r#"
import 'a.dart';

void f(Object? x) {
  if (x case int(foo: true)) {}
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_extension_instance_indexRead"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"a.dart"#,
                r#"
extension E on int {
  int operator[](_) => 0;
}
"#,
            )],
        )],
        source: r#"
import 'a.dart';

void f() {
  0[1];
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_extension_instance_indexReadWrite"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"a.dart"#,
                r#"
extension E on int {
  int operator[](_) => 0;
  void operator[]=(_, __) {}
}
"#,
            )],
        )],
        source: r#"
import 'a.dart';

void f() {
  0[1] += 2;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_extension_instance_indexWrite"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"a.dart"#,
                r#"
extension E on int {
  void operator[]=(_, __) {}
}
"#,
            )],
        )],
        source: r#"
import 'a.dart';

void f() {
  0[1] = 2;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_extension_instance_method"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
extension E on String {
  String empty() => '';
}
"#,
            )],
        )],
        source: r#"
import 'lib1.dart';

f() {
  ''.empty();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_extension_instance_operator_binary"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
extension E on String {
  String operator -(String s) => this;
}
"#,
            )],
        )],
        source: r#"
import 'lib1.dart';

f() {
  'abc' - 'c';
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_extension_instance_operator_unary"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
extension E on String {
  void operator -() {}
}
"#,
            )],
        )],
        source: r#"
import 'lib1.dart';

f() {
  -'abc';
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_extension_instance_setter"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
extension E on String {
  void set foo(int i) {}
}
"#,
            )],
        )],
        source: r#"
import 'lib1.dart';

f() {
  'abc'.foo = 2;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_extension_override_getter"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
extension E on String {
  String get empty => '';
}
"#,
            )],
        )],
        source: r#"
import 'lib1.dart';

f() {
  E('').empty;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_extension_prefixed_isUsed"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
extension E on String {
  String empty() => '';
}
"#,
            )],
        )],
        source: r#"
import 'lib1.dart' as lib1;

f() {
  ''.empty();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_extension_prefixed_notUsed"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
extension E on String {
  String empty() => '';
}
"#,
            )],
        )],
        source: r#"
import 'lib1.dart' as lib1;
//     ^^^^^^^^^^^
// [diag.unusedImport] Unused import: 'lib1.dart'.
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_extension_static_field"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
extension E on String {
  static const String empty = '';
}
"#,
            )],
        )],
        source: r#"
import 'lib1.dart';

f() {
  E.empty;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_hide"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
class A {}
"#,
            )],
        )],
        source: r#"
import 'lib1.dart';
import 'lib1.dart' hide A;
//     ^^^^^^^^^^^
// [diag.unusedImport] Unused import: 'lib1.dart'.
A? a;
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_inComment_libraryDirective"#,
        strict_inference: false,
        packages: &[],
        source: r#"
/// Use [Future] class.
import 'dart:async';
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_metadata"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
const x = 0;
"#,
            )],
        )],
        source: r#"
@A(x)
import 'lib1.dart';
class A {
  final int value;
  const A(this.value);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_multipleExtensions"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[
                (
                    r#"lib1.dart"#,
                    r#"
extension E on String {
  String a() => '';
}
"#,
                ),
                (
                    r#"lib2.dart"#,
                    r#"
extension E on String {
  String b() => '';
}
"#,
                ),
            ],
        )],
        source: r#"
import 'lib1.dart';
//     ^^^^^^^^^^^
// [diag.unusedImport] Unused import: 'lib1.dart'.
import 'lib2.dart';

f() {
  ''.b();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_noPrefix_constructorName_name"#,
        strict_inference: false,
        packages: &[],
        source: r#"
import 'dart:async';
//     ^^^^^^^^^^^^
// [diag.unusedImport] Unused import: 'dart:async'.

class A {
  A.foo();
}

void f() {
  A.foo();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_noPrefix_named_argument"#,
        strict_inference: false,
        packages: &[],
        source: r#"
import 'dart:math';
//     ^^^^^^^^^^^
// [diag.unusedImport] Unused import: 'dart:math'.

void f() {
  Duration(seconds: 0);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_prefixed"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
class A {}
"#,
            )],
        )],
        source: r#"
import 'lib1.dart';
//     ^^^^^^^^^^^
// [diag.unusedImport] Unused import: 'lib1.dart'.
import 'lib1.dart' as one;
one.A a = one.A();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_prefixed_commentReference_prefix"#,
        strict_inference: false,
        packages: &[],
        source: r#"
import 'dart:math' as math;

/// [math]
void f() {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_prefixed_commentReference_prefixClass"#,
        strict_inference: false,
        packages: &[],
        source: r#"
import 'dart:math' as math;

/// [math.Random]
void f() {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_prefixed_samePrefix_notUsed"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[
                (
                    r#"lib1.dart"#,
                    r#"
class A {}
"#,
                ),
                (
                    r#"lib2.dart"#,
                    r#"
class B {}
"#,
                ),
            ],
        )],
        source: r#"
import 'lib1.dart' as one;
import 'lib2.dart' as one;
//     ^^^^^^^^^^^
// [diag.unusedImport] Unused import: 'lib2.dart'.
one.A a = one.A();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_prefixed_samePrefix_referenced"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[
                (
                    r#"lib1.dart"#,
                    r#"
class A {}
"#,
                ),
                (
                    r#"lib2.dart"#,
                    r#"
class B {}
"#,
                ),
            ],
        )],
        source: r#"
import 'lib1.dart' as one;
import 'lib2.dart' as one;
one.A a = one.A();
one.B b = one.B();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_prefixed_samePrefix_referenced_via_export"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[
                (
                    r#"lib1.dart"#,
                    r#"
class A {}
"#,
                ),
                (
                    r#"lib2.dart"#,
                    r#"
class B {}
"#,
                ),
                (
                    r#"lib3.dart"#,
                    r#"
export 'lib2.dart';
"#,
                ),
            ],
        )],
        source: r#"
import 'lib1.dart' as one;
import 'lib3.dart' as one;
one.A a = one.A();
one.B b = one.B();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_prefixed_show_multipleElements"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
class A {}
class B {}
"#,
            )],
        )],
        source: r#"
import 'lib1.dart' as one show A, B;
one.A a = one.A();
one.B b = one.B();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_prefixed_showTopLevelFunction"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
class One {}
topLevelFunction() {}
"#,
            )],
        )],
        source: r#"
import 'lib1.dart' hide topLevelFunction;
import 'lib1.dart' as one show topLevelFunction;
class A {
  static void x() {
    One o;
//      ^
// [diag.unusedLocalVariable] The value of the local variable 'o' isn't used.
    one.topLevelFunction();
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_prefixed_showTopLevelFunction_multipleDirectives"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
class One {}
topLevelFunction() {}
"#,
            )],
        )],
        source: r#"
import 'lib1.dart' hide topLevelFunction;
import 'lib1.dart' as one show topLevelFunction;
import 'lib1.dart' as two show topLevelFunction;
class A {
  static void x(One o) {
    one.topLevelFunction();
    two.topLevelFunction();
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_prefixed_systemLibrary"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"a.dart"#,
                r#"
class File {}
"#,
            )],
        )],
        source: r#"
import 'dart:io' as prefix;
//     ^^^^^^^^^
// [diag.unusedImport] Unused import: 'dart:io'.
import 'a.dart' as prefix;
prefix.File? f;
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_show"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
class A {}
class B {}
"#,
            )],
        )],
        source: r#"
import 'lib1.dart' show A;
import 'lib1.dart' show B;
//     ^^^^^^^^^^^
// [diag.unusedImport] Unused import: 'lib1.dart'.
A a = A();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_import_test.dart::UnusedImportTest::test_library_systemLibrary"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
class File {}
"#,
            )],
        )],
        source: r#"
import 'dart:io';
//     ^^^^^^^^^
// [diag.unusedImport] Unused import: 'dart:io'.
import 'lib1.dart';
File? f;
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_forEachPartsWithPattern_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(List<(int,)> x) {
  for (var (a,) in x) {}
//          ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_forEachPartsWithPattern_used"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(List<(int,)> x) {
  for (var (a,) in x) {
    a;
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_forEachPartsWithPattern_wildcard"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(List<(int,)> x) {
  for (var (_,) in x) {}
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_forPartsWithPattern_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  for (var (a,) = (0,);;) {}
//          ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_forPartsWithPattern_used"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  for (var (a,) = (0,);;) {
    a;
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_forPartsWithPattern_wildcard"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  for (var (_,) = (0,);;) {}
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_ifStatement_caseClause_logicalOr_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(Object? x) {
  if (x case int a || [int a]) {}
//               ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
//                         ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_ifStatement_caseClause_logicalOr_used"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(Object? x) {
  if (x case int a || [int a]) {
    a;
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_ifStatement_caseClause_single_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(Object? x) {
  if (x case int a) {}
//               ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_ifStatement_caseClause_single_used"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(Object? x) {
  if (x case int a) {
    a;
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_ifStatement_caseClause_whenClause"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(Object? x) {
  if (x case int a when a > 0) {}
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_ifStatement_caseClause_wildcard"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(Object? x) {
  if (x case int _) {}
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_inFor_underscores"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  for (var _ in [1,2,3]) {
    for (var __ in [4,5,6]) {
//           ^^
// [diag.unusedLocalVariable] The value of the local variable '__' isn't used.
      // do something
    }
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_inFor_underscores_preWildCards"#,
        strict_inference: false,
        packages: &[],
        source: r#"
// @dart = 3.4
// (pre wildcard-variables)
f() {
  for (var _ in [1,2,3]) {
    for (var __ in [4,5,6]) {
      // do something
    }
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_localVariable_forElement_underscores"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
    [
      for (var __ in [1, 2, 3]) 1
//             ^^
// [diag.unusedLocalVariable] The value of the local variable '__' isn't used.
    ];
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_localVariable_underscores"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  var __ = 0;
//    ^^
// [diag.unusedLocalVariable] The value of the local variable '__' isn't used.
  var ___ = 0;
//    ^^^
// [diag.unusedLocalVariable] The value of the local variable '___' isn't used.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_localVariable_wildcard"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  var _ = 0;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_localVariableListPattern_underscores"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  var [__] = [1];
//     ^^
// [diag.unusedLocalVariable] The value of the local variable '__' isn't used.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_localVariableListPattern_wildcard"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  var [_] = [1];
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_localVariablePattern_underscores"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  var (__) = (1);
//     ^^
// [diag.unusedLocalVariable] The value of the local variable '__' isn't used.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_localVariablePattern_wildcard"#,
        strict_inference: false,
        packages: &[],
        source: r#"
f() {
  var (_) = (1);
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_localVariableSwitchListPattern_underscores"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(Object o) {
  switch(o) {
    case [var __] : {}
//            ^^
// [diag.unusedLocalVariable] The value of the local variable '__' isn't used.
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_localVariableSwitchListPattern_wildcard"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(Object o) {
  switch(o) {
    case [var _] : {}
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_patternVariableDeclarationStatement_noneUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  var (a, b) = (0, 1);
//     ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
//        ^
// [diag.unusedLocalVariable] The value of the local variable 'b' isn't used.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_patternVariableDeclarationStatement_noneUsed_nested"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  var (a, [b, _]) = (0, []);
//     ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
//         ^
// [diag.unusedLocalVariable] The value of the local variable 'b' isn't used.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_patternVariableDeclarationStatement_noneUsed_withChildStatements"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  var (a, b) = () {
//     ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
//        ^
// [diag.unusedLocalVariable] The value of the local variable 'b' isn't used.
    var (c, d) = (0, 1);
    return (c, d);
  }();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_patternVariableDeclarationStatement_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  var (a,) = (0,);
//     ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_patternVariableDeclarationStatement_someUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  var (a, b) = (0, 1);
  a;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_patternVariableDeclarationStatement_someUsed_nested"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  var (a, [b, c]) = (0, []);
  c;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_patternVariableDeclarationStatement_used"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  var (a,) = (0,);
  a;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_patternVariableDeclarationStatement_wildcard"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f() {
  var (a, _) = (0, 1);
  a;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_switchExpression_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
Object? f(Object? x) {
  return switch (x) {
    (int a,) => 0,
//       ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
    _ => 0,
  };
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_switchExpression_used"#,
        strict_inference: false,
        packages: &[],
        source: r#"
Object? f(Object? x) {
  return switch (x) {
    (int a,) => a,
    _ => 0,
  };
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_switchExpression_wildcard"#,
        strict_inference: false,
        packages: &[],
        source: r#"
Object? f(Object? x) {
  return switch (x) {
    (int _,) => 0,
    _ => 0,
  };
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_switchStatement_patternCase_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(Object? x) {
  switch (x) {
    case (var a,):
//            ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
      break;
  };
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_switchStatement_patternCase_used"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(Object? x) {
  switch (x) {
    case (var a,):
      a;
  };
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_switchStatement_patternCase_wildcard"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(Object? x) {
  switch (x) {
    case (int _,):
      break;
  };
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_switchStatement_sharedScope_consistent_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(Object? x) {
  switch (x) {
    case (var a,):
//            ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
    case [var a,]:
//            ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
      break;
  };
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_switchStatement_sharedScope_consistent_used"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(Object? x) {
  switch (x) {
    case (var a,):
    case [var a,]:
      a;
  };
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_switchStatement_sharedScope_notConsistent_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(Object? x) {
  switch (x) {
    case 0:
    case [var a]:
//            ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
      break;
  };
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_switchStatement_sharedScope_notConsistent_used"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(Object? x) {
  switch (x) {
    case 0:
    case [var a]:
      a;
//    ^
// [diag.patternVariableSharedCaseScopeNotAllCases] The variable 'a' is available in some, but not all cases that share this body.
  };
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_switchStatement_sharedScope_whenClause_notUsed_used"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(Object? x) {
  switch (x) {
    case [int a,]:
//            ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
    case (int a,) when a > 0:
      break;
  };
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_switchStatement_sharedScope_whenClause_used_notDeclared"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(Object? x) {
  switch (x) {
    case (int a,) when a > 0:
    case [int _]:
      break;
  };
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_switchStatement_sharedScope_whenClause_used_notUsed"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(Object? x) {
  switch (x) {
    case (int a,) when a > 0:
    case [int a,]:
//            ^
// [diag.unusedLocalVariable] The value of the local variable 'a' isn't used.
      break;
  };
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_switchStatement_sharedScope_whenClause_used_used"#,
        strict_inference: false,
        packages: &[],
        source: r#"
void f(Object? x) {
  switch (x) {
    case (int a,) when a > 0:
    case [int a,] when a > 0:
      break;
  };
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_variableDeclarationStatement_inFunction"#,
        strict_inference: false,
        packages: &[],
        source: r#"
main() {
  var v = 1;
//    ^
// [diag.unusedLocalVariable] The value of the local variable 'v' isn't used.
  v = 2;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_variableDeclarationStatement_inMethod"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  foo() {
    var v = 1;
//      ^
// [diag.unusedLocalVariable] The value of the local variable 'v' isn't used.
    v = 2;
  }
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_variableDeclarationStatement_isInvoked"#,
        strict_inference: false,
        packages: &[],
        source: r#"
typedef Foo();
main() {
  Foo foo = () {};
  foo();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_variableDeclarationStatement_isNullAssigned"#,
        strict_inference: false,
        packages: &[],
        source: r#"
typedef Foo();
main() {
  var v;
  v ??= doSomething();
}
doSomething() => 42;
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_variableDeclarationStatement_isRead_notUsed_compoundAssign"#,
        strict_inference: false,
        packages: &[],
        source: r#"
main() {
  var v = 1;
//    ^
// [diag.unusedLocalVariable] The value of the local variable 'v' isn't used.
  v += 2;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_variableDeclarationStatement_isRead_notUsed_postfixExpr"#,
        strict_inference: false,
        packages: &[],
        source: r#"
main() {
  var v = 1;
//    ^
// [diag.unusedLocalVariable] The value of the local variable 'v' isn't used.
  v++;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_variableDeclarationStatement_isRead_notUsed_prefixExpr"#,
        strict_inference: false,
        packages: &[],
        source: r#"
main() {
  var v = 1;
//    ^
// [diag.unusedLocalVariable] The value of the local variable 'v' isn't used.
  ++v;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_variableDeclarationStatement_isRead_usedArgument"#,
        strict_inference: false,
        packages: &[],
        source: r#"
main() {
  var v = 1;
  print(++v);
}
print(x) {}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_local_variable_test.dart::UnusedLocalVariableTest::test_variableDeclarationStatement_isRead_usedInvocationTarget"#,
        strict_inference: false,
        packages: &[],
        source: r#"
class A {
  foo() {}
}
main() {
  var a = new A();
  a.foo();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_shown_name_test.dart::UnusedShownNameTest::test_dartCore_unused"#,
        strict_inference: false,
        packages: &[],
        source: r#"
import 'dart:core' as core show int;
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_shown_name_test.dart::UnusedShownNameTest::test_extension_instance_method_unused"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
extension E on String {
  String empty() => '';
}
String s = '';
"#,
            )],
        )],
        source: r#"
import 'lib1.dart' show E, s;
//                      ^
// [diag.unusedShownName] The name E is shown, but isn't used.

f() {
  s.length;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_shown_name_test.dart::UnusedShownNameTest::test_extension_instance_method_used"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
extension E on String {
  String empty() => '';
}
"#,
            )],
        )],
        source: r#"
import 'lib1.dart' show E;

f() {
  ''.empty();
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_shown_name_test.dart::UnusedShownNameTest::test_referenced_prefixed_assignmentExpression"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"a.dart"#,
                r#"
var a = 0;
"#,
            )],
        )],
        source: r#"
import 'a.dart' as p show a;

void f() {
  p.a = 0;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_shown_name_test.dart::UnusedShownNameTest::test_referenced_prefixed_postfixExpression"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"a.dart"#,
                r#"
var a = 0;
"#,
            )],
        )],
        source: r#"
import 'a.dart' as p show a;

void f() {
  p.a++;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_shown_name_test.dart::UnusedShownNameTest::test_referenced_prefixed_prefixExpression"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"a.dart"#,
                r#"
var a = 0;
"#,
            )],
        )],
        source: r#"
import 'a.dart' as p show a;

void f() {
  ++p.a;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_shown_name_test.dart::UnusedShownNameTest::test_referenced_unprefixed_assignmentExpression"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"a.dart"#,
                r#"
var a = 0;
"#,
            )],
        )],
        source: r#"
import 'a.dart' show a;

void f() {
  a = 0;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_shown_name_test.dart::UnusedShownNameTest::test_referenced_unprefixed_postfixExpression"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"a.dart"#,
                r#"
var a = 0;
"#,
            )],
        )],
        source: r#"
import 'a.dart' show a;

void f() {
  a++;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_shown_name_test.dart::UnusedShownNameTest::test_referenced_unprefixed_prefixExpression"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"a.dart"#,
                r#"
var a = 0;
"#,
            )],
        )],
        source: r#"
import 'a.dart' show a;

void f() {
  ++a;
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_shown_name_test.dart::UnusedShownNameTest::test_unreferenced"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
class A {}
class B {}
"#,
            )],
        )],
        source: r#"
import 'lib1.dart' show A, B;
//                         ^
// [diag.unusedShownName] The name B is shown, but isn't used.
A a = A();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_shown_name_test.dart::UnusedShownNameTest::test_unreferenced_dotShorthand"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"a.dart"#,
                r#"
class A {}

void f(A a) {}
"#,
            )],
        )],
        source: r#"
import 'a.dart' show A, f;
//                   ^
// [diag.unusedShownName] The name A is shown, but isn't used.

void g() {
  f(.new());
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_shown_name_test.dart::UnusedShownNameTest::test_unresolved"#,
        strict_inference: false,
        packages: &[],
        source: r#"
import 'dart:math' show max, FooBar;
//                           ^^^^^^
// [diag.undefinedShownName] The library 'dart:math' doesn't export a member with the shown name 'FooBar'.
main() {
  print(max(1, 2));
}
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_shown_name_test.dart::UnusedShownNameTest::test_unusedShownName_as"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
class A {}
class B {}
"#,
            )],
        )],
        source: r#"
import 'lib1.dart' as p show A, B;
//                              ^
// [diag.unusedShownName] The name B is shown, but isn't used.
p.A a = p.A();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_shown_name_test.dart::UnusedShownNameTest::test_unusedShownName_duplicates"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
class A {}
class B {}
class C {}
class D {}
"#,
            )],
        )],
        source: r#"
import 'lib1.dart' show A, B;
//                         ^
// [diag.unusedShownName] The name B is shown, but isn't used.
import 'lib1.dart' show C, D;
//                         ^
// [diag.unusedShownName] The name D is shown, but isn't used.
A a = A();
C c = C();
"#,
        expected: None,
    },
    Ported {
        name: r#"unused_shown_name_test.dart::UnusedShownNameTest::test_unusedShownName_topLevelVariable"#,
        strict_inference: false,
        packages: &[(
            "test",
            &[(
                r#"lib1.dart"#,
                r#"
const int var1 = 1;
const int var2 = 2;
const int var3 = 3;
const int var4 = 4;
"#,
            )],
        )],
        source: r#"
import 'lib1.dart' show var1, var2;
import 'lib1.dart' show var3, var4;
//                            ^^^^
// [diag.unusedShownName] The name var4 is shown, but isn't used.
int a = var1;
int b = var2;
int c = var3;
"#,
        expected: None,
    },
];

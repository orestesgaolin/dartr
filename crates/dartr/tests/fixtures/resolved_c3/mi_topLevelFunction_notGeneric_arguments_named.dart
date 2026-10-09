// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_topLevelFunction_notGeneric_arguments_named).

void foo(int a, {required bool b}) {}

void f() {
  foo(0, b: true);
}

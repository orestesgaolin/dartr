// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_inFunction_topLevelFunction_generic).

void foo<T>(T a) {}

void f() {
  foo(0);
}

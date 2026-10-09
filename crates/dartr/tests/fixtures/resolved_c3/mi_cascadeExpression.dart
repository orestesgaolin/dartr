// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_cascadeExpression).

class A {
  void foo() {}
  void bar() {}
}

void f(A a) {
  a..foo()..bar();
}

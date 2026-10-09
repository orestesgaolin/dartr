// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_nullShorting_cascade_firstMethodInvocation).

class A {
  int foo() => 0;
  int bar() => 0;
}

void f(A? a) {
  a?..foo()..bar();
}

// Ported from pkg/analyzer/test/src/dart/resolution/function_expression_invocation_test.dart (test_nullShorting).

abstract class A {
  int Function() get foo;
}

class B {
  void bar(A? a) {
    a?.foo();
  }
}

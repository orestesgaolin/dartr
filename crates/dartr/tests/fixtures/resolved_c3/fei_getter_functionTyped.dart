// Ported from pkg/analyzer/test/src/dart/resolution/function_expression_invocation_test.dart (test_getter_functionTyped).

typedef F = String Function(int a, {int b});

class A {
  F get foo => throw 0;

  void f() {
    foo(1, b: 2);
  }
}

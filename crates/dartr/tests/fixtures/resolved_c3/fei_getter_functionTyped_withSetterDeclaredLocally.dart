// Ported from pkg/analyzer/test/src/dart/resolution/function_expression_invocation_test.dart (test_getter_functionTyped_withSetterDeclaredLocally).

class A {
  Function get foo => () {};
}
class B extends A {
  set foo(Function _) {}

  void f() {
    foo();
  }
}

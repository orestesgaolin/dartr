// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_error_invocationOfNonFunction_super_getter).

class A {
  int get foo => 0;
}

class B extends A {
  main() {
    super.foo();
//  ^^^^^^^^^
// [diag.invocationOfNonFunctionExpression] The expression doesn't evaluate to a function, so it can't be invoked.
  }
}

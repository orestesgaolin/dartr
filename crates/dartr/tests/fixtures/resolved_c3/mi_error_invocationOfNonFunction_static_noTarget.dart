// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_error_invocationOfNonFunction_static_noTarget).

class C {
  static int foo = 0;

  main() {
    foo();
//  ^^^
// [diag.invocationOfNonFunctionExpression] The expression doesn't evaluate to a function, so it can't be invoked.
  }
}

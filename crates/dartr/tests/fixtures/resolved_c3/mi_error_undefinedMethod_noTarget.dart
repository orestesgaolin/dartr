// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_error_undefinedMethod_noTarget).

class C {
  main() {
    foo(0);
//  ^^^
// [diag.undefinedMethod] The method 'foo' isn't defined for the type 'C'.
  }
}

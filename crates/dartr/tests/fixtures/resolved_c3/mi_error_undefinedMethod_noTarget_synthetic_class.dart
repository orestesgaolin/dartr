// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_error_undefinedMethod_noTarget_synthetic_class).

class {
//    ^
// [diag.missingIdentifier] Expected an identifier.
  void f() {
    foo(0);
  }
}

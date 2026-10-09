// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_error_undefinedMethod_hasTarget_class).

class C {}
main() {
  C.foo(0);
//  ^^^
// [diag.undefinedMethod] The method 'foo' isn't defined for the type 'C'.
}

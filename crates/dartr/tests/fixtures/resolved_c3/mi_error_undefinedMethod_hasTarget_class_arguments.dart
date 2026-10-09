// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_error_undefinedMethod_hasTarget_class_arguments).

class C {}

int x = 0;
main() {
  C.foo(x);
//  ^^^
// [diag.undefinedMethod] The method 'foo' isn't defined for the type 'C'.
}

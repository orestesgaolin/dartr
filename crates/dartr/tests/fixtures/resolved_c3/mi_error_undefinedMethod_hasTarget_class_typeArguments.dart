// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_error_undefinedMethod_hasTarget_class_typeArguments).

class C {}

main() {
  C.foo<int>();
//  ^^^
// [diag.undefinedMethod] The method 'foo' isn't defined for the type 'C'.
}

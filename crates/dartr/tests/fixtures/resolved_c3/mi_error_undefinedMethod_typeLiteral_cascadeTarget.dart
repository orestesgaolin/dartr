// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_error_undefinedMethod_typeLiteral_cascadeTarget).

class C {
  static void foo() {}
}

main() {
  C..foo();
//   ^^^
// [diag.undefinedMethod] The method 'foo' isn't defined for the type 'Type'.
}

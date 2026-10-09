// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_error_undefinedMethod_hasTarget_class_inSuperclass).

class S {
  static void foo(int _) {}
}

class C extends S {}

main() {
  C.foo(0);
//  ^^^
// [diag.undefinedMethod] The method 'foo' isn't defined for the type 'C'.
}

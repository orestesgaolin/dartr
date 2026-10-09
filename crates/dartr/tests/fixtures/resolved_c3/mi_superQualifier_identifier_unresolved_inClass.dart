// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_superQualifier_identifier_unresolved_inClass).

class A {}

class B extends A {
  void foo(int _) {
    super.foo(0);
//        ^^^
// [diag.undefinedSuperMethod] The method 'foo' isn't defined in a superclass of 'B'.
  }
}

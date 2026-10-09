// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_superQualifier_identifier_methodOfMixin_inEnum).

mixin M {
  void foo() {}
}

enum E with M {
  v;
  void f() {
    super.foo();
  }
}

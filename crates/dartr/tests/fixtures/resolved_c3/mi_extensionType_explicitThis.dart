// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_extensionType_explicitThis).

extension type A(int it) {
  void foo() {}

  void f() {
    this.foo();
  }
}

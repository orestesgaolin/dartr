// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_interfaceType_extensionType_redeclared).

class A {
  void foo() {}
}

extension type X(A it) implements A {
  void foo() {}
}

void f(X x) {
  x.foo();
}

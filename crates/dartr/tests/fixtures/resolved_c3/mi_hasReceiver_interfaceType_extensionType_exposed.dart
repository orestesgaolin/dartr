// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_interfaceType_extensionType_exposed).

class A {
  void foo() {}
}

class B extends A {}

extension type X(B it) implements A {}

void f(X x) {
  x.foo();
}

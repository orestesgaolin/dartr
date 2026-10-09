// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_interfaceTypeQ_defined_extensionQ).

class A {
  void foo() {}
}

extension E on A? {
  void foo() {}
}

void f(A? a) {
  a.foo();
}

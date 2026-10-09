// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_interfaceType_extensionType_notExposed).

class A {}

class B extends A {
  void foo() {}
}

extension type X(B it) implements A {}

void f(X x) {
  x.foo();
//  ^^^
// [diag.undefinedMethod] The method 'foo' isn't defined for the type 'X'.
}

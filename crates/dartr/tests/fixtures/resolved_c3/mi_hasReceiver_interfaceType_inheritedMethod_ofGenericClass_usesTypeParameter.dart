// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_interfaceType_inheritedMethod_ofGenericClass_usesTypeParameter).

class A<T> {
  T foo() => throw 0;
}

class B extends A<int> {}

void f(B b) {
  b.foo();
}

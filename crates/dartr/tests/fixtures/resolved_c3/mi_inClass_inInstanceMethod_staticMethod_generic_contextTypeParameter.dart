// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_inClass_inInstanceMethod_staticMethod_generic_contextTypeParameter).

class A<T> {
  static E foo<E>(A<E> p) => throw 0;

  void f() {
    foo(this);
  }
}

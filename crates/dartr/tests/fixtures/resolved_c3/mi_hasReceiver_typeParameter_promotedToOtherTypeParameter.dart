// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_typeParameter_promotedToOtherTypeParameter).

abstract class A {}

abstract class B extends A {
  void foo();
}

void f<T extends A, U extends B>(T a) {
  if (a is U) {
    a.foo();
  }
}

// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_instance_typeParameter).

class A {
  void foo(int _) {}
}

class C<T extends A> {
  T a;
  C(this.a);

  main() {
    a.foo(0);
  }
}

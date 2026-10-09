// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_super_class_method).

class A {
  void foo() {}
}

class B extends A {
  void bar() {
    super.foo();
  }
}

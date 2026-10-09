// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_super_mixin_method).

class A {
  void foo() {}
}

mixin M on A {
  void bar() {
    super.foo();
  }
}

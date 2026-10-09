// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_super_getter).

class A {
  double Function(int) get foo => throw Error();
}

class B extends A {
  void bar() {
    super.foo(0);
  }
}

// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_noReceiver_getter_superClass).

class A {
  double Function(int) get foo => throw Error();
}

class B extends A {
  void bar() {
    foo(0);
  }
}

// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_noReceiver_getter_thisClass).

class C {
  double Function(int) get foo => throw Error();

  void bar() {
    foo(0);
  }
}

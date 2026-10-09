// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_extension_staticGetter).

extension A on int {
  static double Function(int) get foo => throw Error();
}

void f() {
  A.foo(0);
}

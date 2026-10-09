// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_class_staticGetter).

class C {
  static double Function(int) get foo => throw Error();
}

main() {
  C.foo(0);
}

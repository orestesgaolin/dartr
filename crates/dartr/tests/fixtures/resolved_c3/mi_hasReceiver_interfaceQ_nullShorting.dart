// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_interfaceQ_nullShorting).

class C {
  C foo() => throw 0;
  C bar() => throw 0;
}

void testShort(C? c) {
  c?.foo().bar();
}

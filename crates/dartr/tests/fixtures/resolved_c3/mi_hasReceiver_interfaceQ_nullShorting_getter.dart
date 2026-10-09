// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_interfaceQ_nullShorting_getter).

abstract class C {
  void Function(C) get foo;
}

void f(C? c) {
  c?.foo(c);
}

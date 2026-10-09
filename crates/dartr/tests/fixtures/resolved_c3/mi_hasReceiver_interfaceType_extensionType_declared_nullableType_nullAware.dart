// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_interfaceType_extensionType_declared_nullableType_nullAware).

extension type A(int it) {
  int foo() => 0;
}

void f(A? a) {
  a?.foo();
}

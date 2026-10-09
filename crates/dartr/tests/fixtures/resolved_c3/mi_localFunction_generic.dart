// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_localFunction_generic).

void f() {
  T g<T, U>(T a, U b) => throw 0;
  g(1, '2');
}

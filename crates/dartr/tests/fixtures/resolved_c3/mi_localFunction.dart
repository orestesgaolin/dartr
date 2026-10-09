// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_localFunction).

void f() {
  double g(int a, String b) => throw 0;
  g(1, '2');
}

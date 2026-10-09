// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_arguments_synthetics).

void f() {
  g(,,);
//  ^
// [diag.missingIdentifier] Expected an identifier.
//   ^
// [diag.missingIdentifier] Expected an identifier.
}

void g(int a, int b) {}

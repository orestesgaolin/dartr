// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_error_wrongNumberOfTypeArgumentsMethod_01).

void foo() {}

main() {
  foo<int>();
//   ^^^^^
// [diag.wrongNumberOfTypeArgumentsElement] The function 'foo' is declared with 0 type parameters, but 1 type arguments are given.
}

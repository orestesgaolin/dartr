// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_error_wrongNumberOfTypeArgumentsMethod_21).

Map<T, U> foo<T extends num, U>() => throw Error();

main() {
  foo<int>();
//   ^^^^^
// [diag.wrongNumberOfTypeArgumentsElement] The function 'foo' is declared with 2 type parameters, but 1 type arguments are given.
}

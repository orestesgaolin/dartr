// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_typeArgumentTypes_generic_typeArguments_wrongNumber).

void foo<T>() {}

main() {
  foo<int, double>();
//   ^^^^^^^^^^^^^
// [diag.wrongNumberOfTypeArgumentsElement] The function 'foo' is declared with 1 type parameters, but 2 type arguments are given.
}

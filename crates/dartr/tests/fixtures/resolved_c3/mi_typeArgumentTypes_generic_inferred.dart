// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_typeArgumentTypes_generic_inferred).

U foo<T, U>(T a) => throw Error();

main() {
  bool v = foo(0);
//     ^
// [diag.unusedLocalVariable] The value of the local variable 'v' isn't used.
}

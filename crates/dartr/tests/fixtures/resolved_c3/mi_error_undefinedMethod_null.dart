// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_error_undefinedMethod_null).

main() {
  null.foo();
//     ^^^
// [diag.invalidUseOfNullValue] An expression whose value is always 'null' can't be dereferenced.
}

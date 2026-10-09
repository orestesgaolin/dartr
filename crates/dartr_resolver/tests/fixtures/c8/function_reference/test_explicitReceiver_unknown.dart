// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_explicitReceiver_unknown).

bar() {
  a.foo<int>;
//^
// [diag.undefinedIdentifier] Undefined name 'a'.
}

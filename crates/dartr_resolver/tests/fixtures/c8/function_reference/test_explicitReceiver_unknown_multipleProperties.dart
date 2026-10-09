// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_explicitReceiver_unknown_multipleProperties).

bar() {
  a.b.foo<int>;
//^
// [diag.undefinedIdentifier] Undefined name 'a'.
}

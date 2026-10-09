// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_topLevelFunction_prefix_unknownPrefix).

bar() {
  prefix.foo<int>;
//^^^^^^
// [diag.undefinedIdentifier] Undefined name 'prefix'.
}

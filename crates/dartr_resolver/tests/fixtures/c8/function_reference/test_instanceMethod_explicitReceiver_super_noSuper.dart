// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceMethod_explicitReceiver_super_noSuper).

bar() {
  super.foo<int>;
//^^^^^
// [diag.superInInvalidContext] Invalid context for 'super' invocation.
}

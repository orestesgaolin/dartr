// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_explicitReceiver_dynamicTyped).

dynamic f() => 1;

foo() {
  f().instanceMethod<int>;
//^^^^^^^^^^^^^^^^^^^^^^^
// [diag.genericMethodTypeInstantiationOnDynamic] A method tear-off on a receiver whose type is 'dynamic' can't have type arguments.
}

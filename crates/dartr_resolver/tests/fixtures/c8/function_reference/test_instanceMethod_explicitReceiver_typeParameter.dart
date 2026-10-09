// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceMethod_explicitReceiver_typeParameter).

bar<T>() {
  T.foo<int>;
//  ^^^
// [diag.undefinedGetter] The getter 'foo' isn't defined for the type 'Type'.
}

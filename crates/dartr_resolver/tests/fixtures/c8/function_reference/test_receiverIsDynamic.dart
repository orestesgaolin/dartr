// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_receiverIsDynamic).

bar(dynamic a) {
  a.foo<int>;
//^^^^^
// [diag.genericMethodTypeInstantiationOnDynamic] A method tear-off on a receiver whose type is 'dynamic' can't have type arguments.
}

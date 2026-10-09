// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_localVariable_typeVariable_nonFunction).

void bar<T>(T foo) {
  foo<int>;
//^^^
// [diag.disallowedTypeInstantiationExpression] Only a generic type, generic function, generic instance method, or generic constructor can have type arguments.
}

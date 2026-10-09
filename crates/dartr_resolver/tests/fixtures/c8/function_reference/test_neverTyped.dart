// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_neverTyped).

external Never get i;

void bar() {
  i<int>;
//^
// [diag.disallowedTypeInstantiationExpression] Only a generic type, generic function, generic instance method, or generic constructor can have type arguments.
}

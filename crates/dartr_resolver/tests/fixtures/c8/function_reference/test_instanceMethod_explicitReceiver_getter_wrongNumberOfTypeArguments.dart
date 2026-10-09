// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceMethod_explicitReceiver_getter_wrongNumberOfTypeArguments).

class A {
  int get foo => 0;
}

void f(A a) {
  // Extra `()` to force reading the type.
  ((a).foo<double>);
//     ^^^
// [diag.disallowedTypeInstantiationExpression] Only a generic type, generic function, generic instance method, or generic constructor can have type arguments.
}

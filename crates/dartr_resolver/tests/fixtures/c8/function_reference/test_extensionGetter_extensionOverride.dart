// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_extensionGetter_extensionOverride).

class A {}

extension E on A {
  int get foo => 0;
}

bar(A a) {
  E(a).foo<int>;
//^^^^^^^^
// [diag.disallowedTypeInstantiationExpression] Only a generic type, generic function, generic instance method, or generic constructor can have type arguments.
}

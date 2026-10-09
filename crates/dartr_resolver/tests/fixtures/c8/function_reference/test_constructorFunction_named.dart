// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_constructorFunction_named).

class A<T> {
  A.foo() {}
}

var x = (A.foo)<int>;

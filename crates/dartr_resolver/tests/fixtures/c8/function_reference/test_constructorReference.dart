// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_constructorReference).

class A<T> {
  A.foo() {}
}

var x = A.foo<int>;
//           ^^^^^
// [diag.wrongNumberOfTypeArgumentsConstructor] The constructor 'A.foo' doesn't have type parameters.

// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_constructorFunction_unnamed).

class A<T> {
  A();
}

var x = (A.new)<int>;

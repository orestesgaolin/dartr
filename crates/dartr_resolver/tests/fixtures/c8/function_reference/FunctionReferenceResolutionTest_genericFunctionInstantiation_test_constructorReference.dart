// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest_genericFunctionInstantiation.test_constructorReference).

class C<T> {
  C(T a);
}
C<int> Function(int) foo() {
  return C.new;
}

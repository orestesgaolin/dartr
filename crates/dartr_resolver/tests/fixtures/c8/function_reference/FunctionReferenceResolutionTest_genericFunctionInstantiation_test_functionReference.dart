// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest_genericFunctionInstantiation.test_functionReference).

typedef Fn = void Function<U>(U);

void Function(int) foo(Fn f) {
  return f;
}

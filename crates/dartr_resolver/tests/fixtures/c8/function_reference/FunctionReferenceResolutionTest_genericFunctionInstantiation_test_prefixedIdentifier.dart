// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest_genericFunctionInstantiation.test_prefixedIdentifier).

class C {
  late void Function<T>(T) f;
}

void Function(int) foo(C c) {
  return c.f;
}

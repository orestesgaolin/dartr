// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_localVariable_typeVariable_functionTyped).

void bar<T extends void Function<U>(U)>(T foo) {
  foo<int>;
}

// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_function_call_typeArgNotMatchingBound).

void foo<T extends num>(T a) {}

void bar() {
  foo.call<String>;
//         ^^^^^^
// [diag.typeArgumentNotMatchingBounds] 'String' doesn't conform to the bound 'num' of the type parameter 'T'.
}

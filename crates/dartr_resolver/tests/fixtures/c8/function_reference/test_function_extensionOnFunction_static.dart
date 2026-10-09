// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_function_extensionOnFunction_static).

void foo<T>(T a) {}

void bar() {
  foo.m<int>;
//    ^
// [diag.undefinedGetter] The getter 'm' isn't defined for the type 'void Function<T>(T)'.
}

extension E on Function {
  static void m<T>(T t) {}
}

// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_extensionMethod_extensionOverride).

class A {
  int foo = 0;
}

extension E on A {
  void foo<T>(T a) {}
}

bar(A a) {
  E(a).foo<int>;
}

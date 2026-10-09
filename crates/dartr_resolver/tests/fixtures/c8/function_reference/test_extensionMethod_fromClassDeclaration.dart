// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_extensionMethod_fromClassDeclaration).

class A {
  bar() {
    foo<int>;
  }
}

extension E on A {
  void foo<T>(T a) {}
}

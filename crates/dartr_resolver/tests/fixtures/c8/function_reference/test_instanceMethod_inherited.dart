// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceMethod_inherited).

class A {
  void foo<T>(T a) {}
}

class B extends A {
  bar() {
    foo<int>;
  }
}

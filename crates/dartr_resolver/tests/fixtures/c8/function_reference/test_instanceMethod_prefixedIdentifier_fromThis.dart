// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceMethod_prefixedIdentifier_fromThis).

class A {
  void bar<T>() {}
}

abstract class B {
  A get foo;
  void f() {
    foo.bar<int>;
  }
}

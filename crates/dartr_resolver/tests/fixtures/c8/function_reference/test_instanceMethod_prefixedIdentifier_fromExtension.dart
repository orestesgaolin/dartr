// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceMethod_prefixedIdentifier_fromExtension).

class A {
  void bar<T>() {}
}

extension on B {
  A get foo => A();
}

abstract class B {
  void f() {
    foo.bar<int>;
  }
}

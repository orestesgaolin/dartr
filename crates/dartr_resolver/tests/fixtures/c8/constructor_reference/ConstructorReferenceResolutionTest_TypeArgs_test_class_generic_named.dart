// Ported from pkg/analyzer/test/src/dart/resolution/constructor_reference_test.dart (ConstructorReferenceResolutionTest_TypeArgs.test_class_generic_named).

class A<T> {
  A.foo();
}

void bar() {
  A<int>.foo;
}

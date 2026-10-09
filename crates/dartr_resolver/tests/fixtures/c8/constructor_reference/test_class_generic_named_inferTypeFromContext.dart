// Ported from pkg/analyzer/test/src/dart/resolution/constructor_reference_test.dart (ConstructorReferenceResolutionTest.test_class_generic_named_inferTypeFromContext).

class A<T> {
  A.foo();
}

A<int> Function() bar() {
  return A.foo;
}

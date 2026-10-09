// Ported from pkg/analyzer/test/src/dart/resolution/constructor_reference_test.dart (ConstructorReferenceResolutionTest.test_class_generic_named_uninstantiated_bound).

class A<T extends num> {
  A.foo();
}

void bar() {
  A.foo;
}

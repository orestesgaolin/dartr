// Ported from pkg/analyzer/test/src/dart/resolution/constructor_reference_test.dart (ConstructorReferenceResolutionTest.test_class_nonGeneric_named).

class A {
  A.foo();
}

void bar() {
  A.foo;
}

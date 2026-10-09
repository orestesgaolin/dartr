// Ported from pkg/analyzer/test/src/dart/resolution/constructor_reference_test.dart (ConstructorReferenceResolutionTest.test_class_nonGeneric_unnamed).

class A {
  A();
}

bar() {
  A.new;
}

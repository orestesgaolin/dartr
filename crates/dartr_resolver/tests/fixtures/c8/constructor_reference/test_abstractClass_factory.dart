// Ported from pkg/analyzer/test/src/dart/resolution/constructor_reference_test.dart (ConstructorReferenceResolutionTest.test_abstractClass_factory).

abstract class A {
  factory A() => A2();
}

class A2 implements A {}

foo() {
  A.new;
}

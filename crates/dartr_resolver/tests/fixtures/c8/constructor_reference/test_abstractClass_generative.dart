// Ported from pkg/analyzer/test/src/dart/resolution/constructor_reference_test.dart (ConstructorReferenceResolutionTest.test_abstractClass_generative).

abstract class A {
  A();
}

foo() {
  A.new;
//^^^^^
// [diag.tearoffOfGenerativeConstructorOfAbstractClass] A generative constructor of an abstract class can't be torn off.
}

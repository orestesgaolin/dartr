// Ported from pkg/analyzer/test/src/dart/resolution/constructor_reference_test.dart (ConstructorReferenceResolutionTest.test_abstractClass_redirecting).

abstract class A {
  A(): this.two();

  A.two();
}

foo() {
  A.new;
//^^^^^
// [diag.tearoffOfGenerativeConstructorOfAbstractClass] A generative constructor of an abstract class can't be torn off.
}

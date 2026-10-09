// Ported from pkg/analyzer/test/src/dart/resolution/constructor_reference_test.dart (ConstructorReferenceResolutionTest_TypeArgs.test_class_generic_nonConstructor).

class A<T> {
  static int i = 1;
}

void bar() {
  A<int>.i;
//^^^^^^^^
// [diag.classInstantiationAccessToStaticMember] The static member 'i' can't be accessed on a class instantiation.
}

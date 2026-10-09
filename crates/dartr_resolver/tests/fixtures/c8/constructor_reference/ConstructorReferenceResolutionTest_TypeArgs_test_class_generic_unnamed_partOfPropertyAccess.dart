// Ported from pkg/analyzer/test/src/dart/resolution/constructor_reference_test.dart (ConstructorReferenceResolutionTest_TypeArgs.test_class_generic_unnamed_partOfPropertyAccess).

class A<T> {
  A();
}

void bar() {
  A<int>.new.runtimeType;
}

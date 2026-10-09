// Ported from pkg/analyzer/test/src/dart/resolution/constructor_reference_test.dart (ConstructorReferenceResolutionTest_TypeArgs.test_class_genericWithBound_unnamed).

class A<T extends num> {
  A();
}

void bar() {
  A<int>.new;
}

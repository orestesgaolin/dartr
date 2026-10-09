// Ported from pkg/analyzer/test/src/dart/resolution/constructor_reference_test.dart (ConstructorReferenceResolutionTest_TypeArgs.test_alias_generic_unnamed).

class A<T> {
  A();
}
typedef TA<T> = A<T>;

void bar() {
  TA<int>.new;
}

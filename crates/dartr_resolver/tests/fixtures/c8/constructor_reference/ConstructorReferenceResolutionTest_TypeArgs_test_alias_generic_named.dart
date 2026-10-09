// Ported from pkg/analyzer/test/src/dart/resolution/constructor_reference_test.dart (ConstructorReferenceResolutionTest_TypeArgs.test_alias_generic_named).

class A<T, U> {
  A.foo();
}
typedef TA<T, U> = A<U, T>;

void bar() {
  TA<int, String>.foo;
}

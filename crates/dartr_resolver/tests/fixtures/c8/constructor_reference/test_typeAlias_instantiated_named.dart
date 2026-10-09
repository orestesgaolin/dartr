// Ported from pkg/analyzer/test/src/dart/resolution/constructor_reference_test.dart (ConstructorReferenceResolutionTest.test_typeAlias_instantiated_named).

class A<T> {
  A.foo();
}
typedef TA = A<int>;

bar() {
  TA.foo;
}

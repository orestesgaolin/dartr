// Ported from pkg/analyzer/test/src/dart/resolution/constructor_reference_test.dart (ConstructorReferenceResolutionTest.test_typeAlias_generic_named_uninstantiated).

class A<T, U> {
  A.foo();
}
typedef TA<U> = A<String, U>;

bar() {
  TA.foo;
}

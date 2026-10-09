// Ported from pkg/analyzer/test/src/dart/resolution/constructor_reference_test.dart (ConstructorReferenceResolutionTest.test_typeAlias_generic_const).

class A<T> {
  const A();
}
typedef TA<T> = A<T>;

const a = TA.new;

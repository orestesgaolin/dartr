// Ported from pkg/analyzer/test/src/dart/resolution/constructor_reference_test.dart (ConstructorReferenceResolutionTest.test_typeAlias_instantiated_const).

class A<T> {
  const A();
}
typedef TA = A<int>;

const a = TA.new;

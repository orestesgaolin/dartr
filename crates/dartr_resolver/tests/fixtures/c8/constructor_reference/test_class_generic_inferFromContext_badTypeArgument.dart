// Ported from pkg/analyzer/test/src/dart/resolution/constructor_reference_test.dart (ConstructorReferenceResolutionTest.test_class_generic_inferFromContext_badTypeArgument).

class A<T extends num> {
  A.foo();
}

A<String> Function() bar() {
// [context 1][column 1][length 9] The inverted type 'A<String>' is also not regular-bounded, so the type is not well-bounded.
//^^^^^^
// [diag.typeArgumentNotMatchingBounds][context 1] 'String' doesn't conform to the bound 'num' of the type parameter 'T'.
  return A.foo;
}

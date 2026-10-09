// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_setLiteral_forElement_body_noPrefix).

class C {}
Set<Object> f() {
  return {for (var _ in [0]) C};
}

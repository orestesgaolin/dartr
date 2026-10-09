// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_listLiteral_forElement_body_noPrefix).

class C {}
List<Object> f() {
  return [for (var _ in [0]) C];
}

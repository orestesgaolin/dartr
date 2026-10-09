// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_mapLiteral_ifElement_value_else_noPrefix).

class C {}
Map<int, Object> f(bool b) {
  return {if (b) 1: C else 2: int};
}

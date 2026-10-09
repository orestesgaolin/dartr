// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_mapLiteral_ifElement_key_else_noPrefix).

class C {}
Map<Object, int> f(bool b) {
  return {if (b) C: 1 else int: 2};
}

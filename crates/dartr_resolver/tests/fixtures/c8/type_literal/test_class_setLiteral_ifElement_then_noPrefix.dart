// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_setLiteral_ifElement_then_noPrefix).

class C {}
Set<Object> f(bool b) {
  return {if (b) C};
}

// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_binaryExpression_rightOperand_ifNull_noPrefix).

class C {}
Type? x;
void f() {
  x ?? C;
}

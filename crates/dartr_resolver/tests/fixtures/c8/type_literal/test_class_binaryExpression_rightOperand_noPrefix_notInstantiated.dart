// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_binaryExpression_rightOperand_noPrefix_notInstantiated).

class C<T> {}
void f() {
  int == C;
}

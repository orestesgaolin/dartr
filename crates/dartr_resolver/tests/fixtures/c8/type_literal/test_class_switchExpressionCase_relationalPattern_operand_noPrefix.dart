// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_switchExpressionCase_relationalPattern_operand_noPrefix).

class C {}
int f(Object x) {
  return switch (x) {
    == C => 0,
    _ => 1,
  };
}

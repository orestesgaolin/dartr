// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_prefixExpression_operand_bang_noPrefix).

class C {}
var x = !C;
//       ^
// [diag.nonBoolNegationExpression] A negation operand must have a static type of 'bool'.

// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_prefixExpression_operand_minus_noPrefix).

class C {}
var x = -C;
//      ^
// [diag.undefinedOperator] The operator 'unary-' isn't defined for the type 'Type'.

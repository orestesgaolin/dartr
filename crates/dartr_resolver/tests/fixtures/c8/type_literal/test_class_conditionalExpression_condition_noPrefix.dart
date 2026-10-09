// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_conditionalExpression_condition_noPrefix).

class C {}
var x = C ? 0 : 1;
//      ^
// [diag.nonBoolCondition] Conditions must have a static type of 'bool'.

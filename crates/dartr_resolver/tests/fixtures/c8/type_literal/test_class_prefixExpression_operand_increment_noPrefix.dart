// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_prefixExpression_operand_increment_noPrefix).

class C {}
void f() {
  ++C;
//  ^
// [diag.assignmentToType] Types can't be assigned a value.
}

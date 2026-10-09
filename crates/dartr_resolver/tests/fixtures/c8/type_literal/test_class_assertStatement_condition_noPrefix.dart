// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_assertStatement_condition_noPrefix).

class C {}
void f() {
  assert(C);
//       ^
// [diag.nonBoolExpression] The expression in an assert must be of type 'bool'.
}

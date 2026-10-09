// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_indexExpression_target_noPrefix).

class C {}
void f(int i) {
  C[i];
// ^^^
// [diag.undefinedOperator] The operator '[]' isn't defined for the type 'Type'.
}

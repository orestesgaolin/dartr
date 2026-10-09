// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_isExpression_expression_noPrefix).

class C {}
bool f() {
  return C is Type;
//       ^^^^^^^^^
// [diag.unnecessaryTypeCheckTrue] Unnecessary type check; the result is always 'true'.
}

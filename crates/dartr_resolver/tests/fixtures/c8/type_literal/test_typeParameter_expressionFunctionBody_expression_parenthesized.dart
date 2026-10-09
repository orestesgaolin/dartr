// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_typeParameter_expressionFunctionBody_expression_parenthesized).

class C<T> {
  Type f() => (T);
}

// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_parenthesizedExpression_expression_noPrefix_instantiated).

class C<T> {}
void f() {
  (C<int>);
}

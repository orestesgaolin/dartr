// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_switchExpression_expression_noPrefix).

class C<T> {}
int f() {
  return switch (C<int>) {
    _ => 0,
  };
}

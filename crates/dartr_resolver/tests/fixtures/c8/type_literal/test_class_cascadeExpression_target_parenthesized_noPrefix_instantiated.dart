// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_cascadeExpression_target_parenthesized_noPrefix_instantiated).

class C<T> {}
void f() {
  (C<int>)..hashCode;
}

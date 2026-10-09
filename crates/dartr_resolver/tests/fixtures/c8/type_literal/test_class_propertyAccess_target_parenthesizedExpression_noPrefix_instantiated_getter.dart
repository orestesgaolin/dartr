// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_propertyAccess_target_parenthesizedExpression_noPrefix_instantiated_getter).

class C<T> {}

void bar() {
  (C<int>).foo;
}

extension E on Type {
  int get foo => 0;
}

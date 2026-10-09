// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_propertyAccess_target_parenthesizedExpression_noPrefix_instantiated_setter).

class C<T> {}

void bar() {
  (C<int>).foo = 7;
}

extension E on Type {
  set foo(int value) {}
}

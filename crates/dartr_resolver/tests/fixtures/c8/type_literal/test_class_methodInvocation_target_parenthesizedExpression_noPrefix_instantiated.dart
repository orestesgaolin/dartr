// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_methodInvocation_target_parenthesizedExpression_noPrefix_instantiated).

class C<T> {}

void bar() {
  (C<int>).foo();
}

extension E on Type {
  void foo() {}
}

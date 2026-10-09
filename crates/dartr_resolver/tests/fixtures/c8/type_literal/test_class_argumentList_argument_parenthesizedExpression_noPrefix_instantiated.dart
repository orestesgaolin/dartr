// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_argumentList_argument_parenthesizedExpression_noPrefix_instantiated).

class C<T> {}
void f(Type t) {}
void g() {
  f((C<int>));
}

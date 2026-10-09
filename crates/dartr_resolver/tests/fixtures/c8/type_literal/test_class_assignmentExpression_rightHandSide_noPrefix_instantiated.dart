// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_assignmentExpression_rightHandSide_noPrefix_instantiated).

class C<T> {}
Type t = int;
void f() {
  t = C<int>;
}

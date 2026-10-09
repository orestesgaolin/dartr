// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_typeParameter_assignmentExpression_rightHandSide).

class C<T> {
  Type t = int;
  void f() {
    t = T;
  }
}

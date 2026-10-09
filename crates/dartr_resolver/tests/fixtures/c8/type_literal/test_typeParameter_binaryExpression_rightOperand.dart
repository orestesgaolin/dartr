// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_typeParameter_binaryExpression_rightOperand).

class C<T> {
  void f() {
    int == T;
  }
}

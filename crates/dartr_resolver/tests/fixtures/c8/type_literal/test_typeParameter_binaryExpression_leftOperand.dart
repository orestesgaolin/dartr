// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_typeParameter_binaryExpression_leftOperand).

class C<T> {
  void f() {
    T == int;
  }
}

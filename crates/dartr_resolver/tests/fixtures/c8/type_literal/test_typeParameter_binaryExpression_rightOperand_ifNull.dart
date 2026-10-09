// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_typeParameter_binaryExpression_rightOperand_ifNull).

class C<T> {
  Object? x;
  void f() {
    x ?? T;
  }
}

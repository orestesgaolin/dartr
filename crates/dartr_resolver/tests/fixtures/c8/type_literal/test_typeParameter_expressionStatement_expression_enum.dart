// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_typeParameter_expressionStatement_expression_enum).

enum E<T> {
  v;
  void foo() {
    T;
  }
}

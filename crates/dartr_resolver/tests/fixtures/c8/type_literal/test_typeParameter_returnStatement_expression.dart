// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_typeParameter_returnStatement_expression).

class C<T> {
  Type f() {
    return T;
  }
}

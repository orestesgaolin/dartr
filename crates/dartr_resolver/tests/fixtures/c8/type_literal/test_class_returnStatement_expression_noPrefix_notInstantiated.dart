// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_returnStatement_expression_noPrefix_notInstantiated).

class C<T> {}
Type f() {
  return C;
}

// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_indexExpression_index_noPrefix).

class C {}
void f(dynamic d) {
  d[C];
}

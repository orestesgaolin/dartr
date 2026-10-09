// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_namedExpression_expression_noPrefix).

class C {}
void f({required Type t}) {}
void g() {
  f(t: C);
}

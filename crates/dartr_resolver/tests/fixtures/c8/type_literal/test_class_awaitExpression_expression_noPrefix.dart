// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_awaitExpression_expression_noPrefix).

class C {}
Future<Type> f() async {
  return await C;
}

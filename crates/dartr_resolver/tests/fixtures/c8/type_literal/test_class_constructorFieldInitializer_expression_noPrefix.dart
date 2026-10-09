// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_constructorFieldInitializer_expression_noPrefix).

class C {}
class A {
  Object o;
  A() : o = C;
}

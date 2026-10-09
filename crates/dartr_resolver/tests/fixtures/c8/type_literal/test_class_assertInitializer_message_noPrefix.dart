// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_assertInitializer_message_noPrefix).

class C {}
class A {
  A() : assert(true, C);
}

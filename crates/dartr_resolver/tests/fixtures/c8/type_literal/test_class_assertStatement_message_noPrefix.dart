// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_assertStatement_message_noPrefix).

class C {}
void f() {
  assert(true, C);
}

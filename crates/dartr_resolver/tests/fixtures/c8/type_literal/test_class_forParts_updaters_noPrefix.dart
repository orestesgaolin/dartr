// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_forParts_updaters_noPrefix).

class C {}
void f(bool b) {
  for (; b; C) {}
}

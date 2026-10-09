// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_ifStatement_condition_noPrefix).

class C {}
void f() {
  if (C) {}
//    ^
// [diag.nonBoolCondition] Conditions must have a static type of 'bool'.
}

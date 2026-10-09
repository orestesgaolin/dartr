// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_whileStatement_condition_noPrefix).

class C {}
void f() {
  while (C) {}
//       ^
// [diag.nonBoolCondition] Conditions must have a static type of 'bool'.
}

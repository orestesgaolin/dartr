// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_forStatement_condition_noPrefix).

class C {}
void f() {
  for (; C; ) {}
//       ^
// [diag.nonBoolCondition] Conditions must have a static type of 'bool'.
}

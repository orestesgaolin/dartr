// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_guardedPattern_whenClause_noPrefix).

class C {}
void f(Object x) {
  switch (x) {
    case _ when C:
//              ^
// [diag.nonBoolCondition] Conditions must have a static type of 'bool'.
      break;
  }
}

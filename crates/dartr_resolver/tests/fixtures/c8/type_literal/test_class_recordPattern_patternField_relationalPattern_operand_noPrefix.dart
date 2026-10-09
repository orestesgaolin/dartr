// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_recordPattern_patternField_relationalPattern_operand_noPrefix).

class C {}
void f(Object x) {
  switch (x) {
    case (== C,):
      break;
    default:
      break;
  }
}

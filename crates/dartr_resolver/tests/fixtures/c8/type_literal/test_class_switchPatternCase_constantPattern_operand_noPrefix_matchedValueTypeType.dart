// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_switchPatternCase_constantPattern_operand_noPrefix_matchedValueTypeType).

class C {}
void f(Type t) {
  switch (t) {
    case C:
      break;
    default:
      break;
  }
}

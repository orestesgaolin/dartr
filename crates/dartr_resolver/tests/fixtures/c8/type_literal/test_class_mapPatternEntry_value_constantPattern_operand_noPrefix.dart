// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_mapPatternEntry_value_constantPattern_operand_noPrefix).

class C {}
void f(Object x) {
  switch (x) {
    case {'k': C}:
      break;
    default:
      break;
  }
}

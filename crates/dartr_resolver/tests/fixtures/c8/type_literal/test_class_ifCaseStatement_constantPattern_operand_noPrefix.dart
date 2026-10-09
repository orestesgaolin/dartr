// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_ifCaseStatement_constantPattern_operand_noPrefix).

class C {}
void f(Object x) {
  if (x case C) {}
}

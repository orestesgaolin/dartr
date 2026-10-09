// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_ifCaseElement_constantPattern_operand_noPrefix).

class C {}
List<int> f(Object x) {
  return [if (x case C) 0];
}

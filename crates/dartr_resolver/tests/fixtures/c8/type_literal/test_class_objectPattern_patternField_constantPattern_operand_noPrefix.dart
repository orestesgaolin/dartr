// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_objectPattern_patternField_constantPattern_operand_noPrefix).

class C {}
class A {
  final Object f;
  const A(this.f);
}
void f(Object x) {
  switch (x) {
    case A(f: C):
      break;
    default:
      break;
  }
}

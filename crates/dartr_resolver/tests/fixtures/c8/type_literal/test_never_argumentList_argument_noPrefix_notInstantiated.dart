// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_never_argumentList_argument_noPrefix_notInstantiated).

void f(Type t) {}
void g() {
  f(Never);
}

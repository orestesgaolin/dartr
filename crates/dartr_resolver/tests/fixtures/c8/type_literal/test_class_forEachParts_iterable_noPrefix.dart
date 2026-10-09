// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_forEachParts_iterable_noPrefix).

class C {}
void f() {
  for (var e in C) {
//              ^
// [diag.forInOfInvalidType] The type 'Type' used in the 'for' loop must implement 'Iterable'.
    e;
  }
}

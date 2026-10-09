// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_argumentList_argument_noPrefix_notInstantiated).

class C<T> {}
void f(Type t) {}
void g() {
  f(C);
}

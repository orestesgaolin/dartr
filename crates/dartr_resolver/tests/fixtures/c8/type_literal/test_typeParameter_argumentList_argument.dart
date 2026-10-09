// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_typeParameter_argumentList_argument).

class C<T> {
  void f(Type t) {}
  void g() {
    f(T);
  }
}

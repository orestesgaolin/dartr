// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_typeParameter_listLiteral_ifElement_else).

class C<T> {
  List<Object> f(bool b) {
    return [if (b) int else T];
  }
}

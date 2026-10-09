// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_typeParameter_mapLiteral_ifElement_key_else).

class C<T> {
  void f(bool b) {
    var m = {if (b) T: 1 else Never: 2};
//      ^
// [diag.unusedLocalVariable] The value of the local variable 'm' isn't used.
  }
}

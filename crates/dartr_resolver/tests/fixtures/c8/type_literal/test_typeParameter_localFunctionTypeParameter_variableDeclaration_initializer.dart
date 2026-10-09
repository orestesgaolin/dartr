// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_typeParameter_localFunctionTypeParameter_variableDeclaration_initializer).

void f() {
  void g<U>() {
//     ^
// [diag.unusedElement] The declaration 'g' isn't referenced.
    var x = U;
//      ^
// [diag.unusedLocalVariable] The value of the local variable 'x' isn't used.
  }
}

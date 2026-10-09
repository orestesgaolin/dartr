// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_variableDeclaration_initializer_noPrefix_instantiated_tooManyTypeArgs).

class C<T> {}
var t = C<int, int>;
//       ^^^^^^^^^^
// [diag.wrongNumberOfTypeArguments] The type 'C' is declared with 1 type parameters, but 2 type arguments were given.

// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_variableDeclaration_initializer_noPrefix_instantiated_tooFewTypeArgs).

class C<T, U> {}
var t = C<int>;
//       ^^^^^
// [diag.wrongNumberOfTypeArguments] The type 'C' is declared with 2 type parameters, but 1 type arguments were given.

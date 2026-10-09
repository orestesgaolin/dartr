// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_typeAlias_variableDeclaration_initializer_noPrefix_instantiated_tooFewTypeArgs).

typedef Fn<T, U> = void Function(T, U);
var t = Fn<int>;
//        ^^^^^
// [diag.wrongNumberOfTypeArguments] The type 'Fn' is declared with 2 type parameters, but 1 type arguments were given.

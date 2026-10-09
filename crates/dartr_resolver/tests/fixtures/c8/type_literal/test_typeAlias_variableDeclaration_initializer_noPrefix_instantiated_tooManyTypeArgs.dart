// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_typeAlias_variableDeclaration_initializer_noPrefix_instantiated_tooManyTypeArgs).

typedef Fn<T> = void Function(T);
var t = Fn<int, String>;
//        ^^^^^^^^^^^^^
// [diag.wrongNumberOfTypeArguments] The type 'Fn' is declared with 1 type parameters, but 2 type arguments were given.

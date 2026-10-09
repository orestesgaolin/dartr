// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_typeAlias_variableDeclaration_initializer_noPrefix_instantiated_typeArgumentsDoNotMatchBound).

typedef Fn<T extends num> = void Function(T);
var t = Fn<String>;
//      ^^^^^^^^^^
// [context 1] The inverted type 'Fn<String>' is also not regular-bounded, so the type is not well-bounded.
//         ^^^^^^
// [diag.typeArgumentNotMatchingBounds][context 1] 'String' doesn't conform to the bound 'num' of the type parameter 'T'.

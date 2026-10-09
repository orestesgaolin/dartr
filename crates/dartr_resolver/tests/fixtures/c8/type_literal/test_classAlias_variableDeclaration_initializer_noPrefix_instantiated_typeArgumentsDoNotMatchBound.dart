// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_classAlias_variableDeclaration_initializer_noPrefix_instantiated_typeArgumentsDoNotMatchBound).

class C<T> {}
typedef CA<T extends num> = C<T>;
var t = CA<String>;
//      ^^^^^^^^^^
// [context 1] The inverted type 'CA<String>' is also not regular-bounded, so the type is not well-bounded.
//         ^^^^^^
// [diag.typeArgumentNotMatchingBounds][context 1] 'String' doesn't conform to the bound 'num' of the type parameter 'T'.

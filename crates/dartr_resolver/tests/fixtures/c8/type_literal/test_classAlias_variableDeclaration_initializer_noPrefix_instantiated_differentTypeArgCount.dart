// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_classAlias_variableDeclaration_initializer_noPrefix_instantiated_differentTypeArgCount).

class C<T, U> {}
typedef CA<T> = C<T, int>;
var t = CA<String>;

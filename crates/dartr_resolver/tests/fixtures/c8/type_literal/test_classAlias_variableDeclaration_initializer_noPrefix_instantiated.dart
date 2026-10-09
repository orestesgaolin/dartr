// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_classAlias_variableDeclaration_initializer_noPrefix_instantiated).

class C<T> {}
typedef CA<T> = C<T>;
var t = CA<int>;

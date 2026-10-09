// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_typeAlias_variableDeclaration_initializer_noPrefix_instantiated).

typedef Fn<T> = void Function(T);
var t = Fn<int>;

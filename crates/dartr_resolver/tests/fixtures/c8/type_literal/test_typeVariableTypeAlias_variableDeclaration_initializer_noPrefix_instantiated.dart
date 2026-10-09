// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_typeVariableTypeAlias_variableDeclaration_initializer_noPrefix_instantiated).

typedef T<E> = E;
var t = T<int>;

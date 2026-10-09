// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_extensionType_variableDeclaration_initializer_noPrefix_instantiated).

extension type A<T>(T it) {}
var t = A<int>;

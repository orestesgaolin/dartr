// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_dynamic_variableDeclaration_initializer_noPrefix_hasTypeArguments).

var t = dynamic<int>;
//      ^^^^^^^
// [diag.disallowedTypeInstantiationExpression] Only a generic type, generic function, generic instance method, or generic constructor can have type arguments.

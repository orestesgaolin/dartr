// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_conditionalExpression_thenExpression_noPrefix_instantiated).

class C<T> {}
bool b = true;
var y = b ? C<int> : int;

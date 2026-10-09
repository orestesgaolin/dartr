// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_conditionalExpression_elseExpression_noPrefix_instantiated).

class C<T> {}
bool b = true;
var y = b ? int : C<int>;

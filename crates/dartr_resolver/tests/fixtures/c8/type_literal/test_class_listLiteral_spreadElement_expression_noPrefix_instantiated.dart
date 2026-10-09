// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_listLiteral_spreadElement_expression_noPrefix_instantiated).

class C<T> {}
var l = [...C<int>];
//          ^^^^^^
// [diag.notIterableSpread] Spread elements in list or set literals must implement 'Iterable'.

// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_setLiteral_spreadElement_expression_noPrefix_instantiated).

class C<T> {}
Set<Object> s = {...C<int>};
//                  ^^^^^^
// [diag.notIterableSpread] Spread elements in list or set literals must implement 'Iterable'.

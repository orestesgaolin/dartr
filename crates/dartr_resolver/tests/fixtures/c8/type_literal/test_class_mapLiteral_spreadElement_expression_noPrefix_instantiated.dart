// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_mapLiteral_spreadElement_expression_noPrefix_instantiated).

class C<T> {}
Map<Object, Object> m = {...C<int>};
//                          ^^^^^^
// [diag.notMapSpread] Spread elements in map literals must implement 'Map'.

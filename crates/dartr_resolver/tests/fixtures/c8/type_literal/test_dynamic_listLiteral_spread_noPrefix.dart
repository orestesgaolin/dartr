// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_dynamic_listLiteral_spread_noPrefix).

var l = [...dynamic];
//          ^^^^^^^
// [diag.notIterableSpread] Spread elements in list or set literals must implement 'Iterable'.

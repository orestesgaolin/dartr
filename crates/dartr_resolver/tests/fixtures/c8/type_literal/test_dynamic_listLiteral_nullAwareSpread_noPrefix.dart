// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_dynamic_listLiteral_nullAwareSpread_noPrefix).

var l = [...?dynamic];
//       ^^^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?...' is unnecessary.
//           ^^^^^^^
// [diag.notIterableSpread] Spread elements in list or set literals must implement 'Iterable'.

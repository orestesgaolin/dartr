// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_listLiteral_spreadElement_expression_nullAware_noPrefix_instantiated).

class C<T> {}
var l = [...?C<int>];
//       ^^^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?...' is unnecessary.
//           ^^^^^^
// [diag.notIterableSpread] Spread elements in list or set literals must implement 'Iterable'.

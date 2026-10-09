// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_class_mapLiteral_spreadElement_expression_nullAware_noPrefix_instantiated).

class C<T> {}
Map<Object, Object> m = {...?C<int>};
//                       ^^^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?...' is unnecessary.
//                           ^^^^^^
// [diag.notMapSpread] Spread elements in map literals must implement 'Map'.

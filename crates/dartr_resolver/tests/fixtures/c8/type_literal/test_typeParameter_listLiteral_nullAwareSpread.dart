// Ported from pkg/analyzer/test/src/dart/resolution/type_literal_test.dart (TypeLiteralResolutionTest.test_typeParameter_listLiteral_nullAwareSpread).

class C<T> {
  var l = [...?T];
//         ^^^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?...' is unnecessary.
//             ^
// [diag.notIterableSpread] Spread elements in list or set literals must implement 'Iterable'.
}

// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_error_undefinedMethod_typeLiteral_conditional).

class A {}
main() {
  A?.toString();
// ^^
// [diag.invalidNullAwareOperator] The receiver can't be null, so the null-aware operator '?.' is unnecessary.
//   ^^^^^^^^
// [diag.undefinedMethod] The method 'toString' isn't defined for the type 'A'.
}

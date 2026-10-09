// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_invalid_inDefaultValue_nullAware).

void f({a = b?.foo()}) {}
//          ^
// [diag.undefinedIdentifier] Undefined name 'b'.

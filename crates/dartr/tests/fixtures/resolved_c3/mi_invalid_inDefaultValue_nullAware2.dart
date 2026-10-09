// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_invalid_inDefaultValue_nullAware2).

typedef void F({a = b?.foo()});
//                ^
// [diag.defaultValueInFunctionType] Parameters in a function type can't have default values.
//                  ^
// [diag.undefinedIdentifier] Undefined name 'b'.


typedef void F({a = b?[0]});
//                ^
// [diag.defaultValueInFunctionType] Parameters in a function type can't have default values.
//                  ^
// [diag.undefinedIdentifier] Undefined name 'b'.

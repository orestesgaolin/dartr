
const c = true ? x : 0;
//               ^
// [diag.undefinedIdentifier] Undefined name 'x'.
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
//                   ^
// [diag.deadCode] Dead code.

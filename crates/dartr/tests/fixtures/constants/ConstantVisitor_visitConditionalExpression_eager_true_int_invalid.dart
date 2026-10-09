
const c = true ? 1 : x;
//                   ^
// [diag.undefinedIdentifier] Undefined name 'x'.
// [diag.deadCode] Dead code.
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.

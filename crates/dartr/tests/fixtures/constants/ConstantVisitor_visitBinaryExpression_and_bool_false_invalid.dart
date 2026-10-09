
final a = false;
const c = false && a;
//              ^^^^
// [diag.deadCode] Dead code.
//                 ^
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.

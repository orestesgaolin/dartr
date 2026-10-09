
final a = false;
const c = true || a;
//             ^^^^
// [diag.deadCode] Dead code.
//                ^
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.

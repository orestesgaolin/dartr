
final x = 0;
const c = x ?? 1;
//        ^
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
//          ^^^^
// [diag.deadCode] Dead code.
//             ^
// [diag.deadNullAwareExpression] The left operand can't be null, so the right operand is never executed.

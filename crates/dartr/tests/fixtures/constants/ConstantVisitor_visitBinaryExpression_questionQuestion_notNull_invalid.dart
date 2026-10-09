
final x = 1;
const c = 0 ?? x;
//          ^^^^
// [diag.deadCode] Dead code.
//             ^
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
// [diag.deadNullAwareExpression] The left operand can't be null, so the right operand is never executed.

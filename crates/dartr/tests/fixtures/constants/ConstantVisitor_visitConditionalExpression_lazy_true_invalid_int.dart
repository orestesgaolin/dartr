
const c = true ? new C() : 0;
//               ^^^^^^^
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
//                         ^
// [diag.deadCode] Dead code.
class C {}

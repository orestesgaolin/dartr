
const c = false ? 1 : new C();
//                ^
// [diag.deadCode] Dead code.
//                    ^^^^^^^
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
//                        ^
// [diag.newWithNonType] The name 'C' isn't a class.


const c = null ?? new C();
//                ^^^^^^^
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
class C {}

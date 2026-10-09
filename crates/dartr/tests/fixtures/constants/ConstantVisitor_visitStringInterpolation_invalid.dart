
const c = 'a${f()}c';
//            ^
// [diag.undefinedFunction] The function 'f' isn't defined.
//            ^^^
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.

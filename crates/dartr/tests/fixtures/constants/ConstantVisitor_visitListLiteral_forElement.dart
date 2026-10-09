
const x = [for (int i = 0; i < 3; i++) i];
//        ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
//         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.constEvalForElement] Constant expressions don't support 'for' elements.

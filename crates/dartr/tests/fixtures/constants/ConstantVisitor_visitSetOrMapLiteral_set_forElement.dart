
const Set set = {};
const x = {for (final i in set) i};
//        ^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
//         ^^^^^^^^^^^^^^^^^^^^^^
// [diag.constEvalForElement] Constant expressions don't support 'for' elements.

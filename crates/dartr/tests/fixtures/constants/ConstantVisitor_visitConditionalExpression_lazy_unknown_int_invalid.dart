
const c = identical(0, 0.0) ? 1 : new Object();
//                                ^^^^^^^^^^^^
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.

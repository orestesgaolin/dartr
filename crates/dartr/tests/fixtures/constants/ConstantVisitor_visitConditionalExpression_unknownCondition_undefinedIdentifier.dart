
const bool kIsWeb = identical(0, 0.0);
const x = kIsWeb ? a : b;
//                 ^
// [diag.undefinedIdentifier] Undefined name 'a'.
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
//                     ^
// [diag.undefinedIdentifier] Undefined name 'b'.


const bool kIsWeb = identical(0, 0.0);

var a = 2;
const x = A(kIsWeb ? 0 : a);
//                       ^
// [diag.invalidConstant] Invalid constant value.
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.

class A {
  const A(int _);
}

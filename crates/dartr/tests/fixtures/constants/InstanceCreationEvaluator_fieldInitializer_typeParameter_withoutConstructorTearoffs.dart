
// @dart=2.12
class A<T> {
  final Object f;
  const A(): f = T;
//               ^
// [context 1] The error is in the field initializer of 'A', and occurs here.
// [diag.invalidConstant] Invalid constant value.
}
const a = const A<int>();
//        ^^^^^^^^^^^^^^
// [diag.constTypeParameter][context 1] Type parameters can't be used in a constant expression.

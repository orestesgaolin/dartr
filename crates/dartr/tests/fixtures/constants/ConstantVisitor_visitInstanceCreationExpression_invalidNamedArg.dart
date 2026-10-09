
class A {
  const A({ required int x });
}
const a = A(x: false);
//          ^^^^^^^^
// [diag.constConstructorParamTypeMismatch] A value of type 'bool' can't be assigned to a parameter of type 'int' in a const constructor.
//             ^^^^^
// [diag.argumentTypeNotAssignable] The argument type 'bool' can't be assigned to the parameter type 'int'.

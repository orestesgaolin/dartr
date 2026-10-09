
class A {
  const A(int x);
}
const a = A(false);
//          ^^^^^
// [diag.argumentTypeNotAssignable] The argument type 'bool' can't be assigned to the parameter type 'int'.
// [diag.constConstructorParamTypeMismatch] A value of type 'bool' can't be assigned to a parameter of type 'int' in a const constructor.

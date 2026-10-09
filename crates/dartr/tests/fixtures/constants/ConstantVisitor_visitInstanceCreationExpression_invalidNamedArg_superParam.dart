
class A {
  const A({ required int x });
}
class B extends A {
  const B({ required super.x });
}
const a = B(x: false);
//          ^^^^^^^^
// [diag.constConstructorParamTypeMismatch] A value of type 'bool' can't be assigned to a parameter of type 'int' in a const constructor.
//             ^^^^^
// [diag.argumentTypeNotAssignable] The argument type 'bool' can't be assigned to the parameter type 'int'.


class C {
  const C();
  String operator +(String other) => other;
}

const c = C() + 1;
//        ^^^^^^^
// [diag.constEvalTypeNumString] In constant expressions, operands of this operator must be of type 'num' or 'String'.
//              ^
// [diag.argumentTypeNotAssignable] The argument type 'int' can't be assigned to the parameter type 'String'.

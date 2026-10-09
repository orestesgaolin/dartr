
// @dart = 2.19
class A {
  const A();
}

const v = A() == 0;
//        ^^^^^^^^
// [diag.constEvalTypeBoolNumString] In constant expressions, operands of this operator must be of type 'bool', 'num', 'String' or 'null'.

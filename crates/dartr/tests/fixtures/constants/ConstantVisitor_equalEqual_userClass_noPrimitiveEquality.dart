
class A {
  const A();
  bool operator ==(other) => false;
}

const v = A() == A();
//        ^^^^^^^^^^
// [diag.constEvalPrimitiveEquality] In constant expressions, operands of the equality operator must have primitive equality.

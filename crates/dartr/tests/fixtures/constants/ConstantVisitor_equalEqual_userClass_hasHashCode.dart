
class A {
  const A();
  int get hashCode => 0;
}

const v = A() == 0;
//        ^^^^^^^^
// [diag.constEvalPrimitiveEquality] In constant expressions, operands of the equality operator must have primitive equality.

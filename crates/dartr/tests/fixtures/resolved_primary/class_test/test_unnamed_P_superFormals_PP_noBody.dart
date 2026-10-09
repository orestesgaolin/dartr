class A {
  A(int p1);
}
class B(super.p1, super.p2) extends A;
//                      ^^
// [diag.superFormalParameterWithoutAssociatedPositional] No associated positional super constructor parameter.

class A {
  A({int? n1});
}
class B(super.p1) extends A;
//            ^^
// [diag.superFormalParameterWithoutAssociatedPositional] No associated positional super constructor parameter.

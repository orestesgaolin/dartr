class A {
  A(int p1, int p2);
}
class B(super.p1) extends A {
  this : super();
//             ^
// [diag.notEnoughPositionalArgumentsNamePlural] 2 positional arguments expected by 'A.new', but 1 found.
}

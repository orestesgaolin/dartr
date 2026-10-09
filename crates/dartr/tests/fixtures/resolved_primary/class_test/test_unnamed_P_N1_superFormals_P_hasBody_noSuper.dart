class A {
  A(int p1, {required int n1});
}
class B(super.p1) extends A {
  this;
//^^^^
// [diag.implicitSuperInitializerMissingArguments] The implicitly invoked unnamed constructor from 'A' has required parameters.
}

class A {
  A(int p1, int p2);
}
class B(super.p1) extends A;
//    ^
// [diag.implicitSuperInitializerMissingArguments] The implicitly invoked unnamed constructor from 'A' has required parameters.

class A {
  A({required int n1});
}
class B(super.n1) extends A {
//            ^^
// [diag.superFormalParameterWithoutAssociatedPositional] No associated positional super constructor parameter.
  this;
//^^^^
// [diag.implicitSuperInitializerMissingArguments] The implicitly invoked unnamed constructor from 'A' has required parameters.
}

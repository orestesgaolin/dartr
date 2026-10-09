class A {
  A({required int n1, required int n2});
}
class B({required super.n1}) extends A {
  this;
//^^^^
// [diag.implicitSuperInitializerMissingArguments] The implicitly invoked unnamed constructor from 'A' has required parameters.
}

class A {
  A.named({required int n1});
}
class B.named({required super.n1}) extends A {
  this : super.named(n1: 0);
//                   ^^
// [diag.duplicateNamedArgument] The argument for the named parameter 'n1' was already specified.
}

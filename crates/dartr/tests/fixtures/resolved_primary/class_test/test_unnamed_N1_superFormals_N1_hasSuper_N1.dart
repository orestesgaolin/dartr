class A {
  A({int? n1});
}
class B({super.n1}) extends A {
  this : super(n1: 1);
//             ^^
// [diag.duplicateNamedArgument] The argument for the named parameter 'n1' was already specified.
}

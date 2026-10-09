class A {
  A({required int n1, required int n2});
}
class B({required super.n1}) extends A {
  this : super();
//       ^^^^^^^
// [diag.missingRequiredArgument] The named parameter 'n2' is required, but there's no corresponding argument.
}

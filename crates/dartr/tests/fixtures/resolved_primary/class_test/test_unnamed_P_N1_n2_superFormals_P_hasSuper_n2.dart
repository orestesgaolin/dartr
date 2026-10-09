class A {
  A(int p1, {required int n1, int? n2});
}
class B(super.p1) extends A {
  this : super(n2: 1);
//       ^^^^^^^^^^^^
// [diag.missingRequiredArgument] The named parameter 'n1' is required, but there's no corresponding argument.
}

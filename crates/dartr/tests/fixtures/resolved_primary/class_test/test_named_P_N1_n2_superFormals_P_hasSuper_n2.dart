class A {
  A.named(int p1, {required int n1, int? n2});
}
class B.named(super.p1) extends A {
  this : super.named(n2: 1);
//       ^^^^^^^^^^^^^^^^^^
// [diag.missingRequiredArgument] The named parameter 'n1' is required, but there's no corresponding argument.
}

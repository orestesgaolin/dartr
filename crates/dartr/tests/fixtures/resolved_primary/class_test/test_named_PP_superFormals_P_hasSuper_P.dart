class A {
  A.named(int p1, int p2);
}
class B.named(super.p1) extends A {
//                  ^^
// [diag.positionalSuperFormalParameterWithPositionalArgument] Positional super parameters can't be used when the super constructor invocation has a positional argument.
  this : super.named(0);
}

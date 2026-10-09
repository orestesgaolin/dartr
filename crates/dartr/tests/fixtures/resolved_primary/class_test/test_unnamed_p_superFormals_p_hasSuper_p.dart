class A {
  A([int? p1]);
}
class B(super.p1) extends A {
//            ^^
// [diag.positionalSuperFormalParameterWithPositionalArgument] Positional super parameters can't be used when the super constructor invocation has a positional argument.
  this : super(1);
}

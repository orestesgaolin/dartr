class A {
  A(int p1);
}
class B(String super.p1) extends A;
//                   ^^
// [diag.superFormalParameterTypeIsNotSubtypeOfAssociated] The type 'String' of this parameter isn't a subtype of the type 'int' of the associated super constructor parameter.

class A {
  A([int? p1]);
}
class B({super.n1}) extends A;
//             ^^
// [diag.superFormalParameterWithoutAssociatedNamed] No associated named super constructor parameter.

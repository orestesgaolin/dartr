class A {
  A(int p1);
}
class B([super.p1]) extends A {
//             ^^
// [diag.missingDefaultValueForParameterPositional] The parameter 'p1' can't have a value of 'null' because of its type, but the implicit default value is 'null'.
  this : super();
}

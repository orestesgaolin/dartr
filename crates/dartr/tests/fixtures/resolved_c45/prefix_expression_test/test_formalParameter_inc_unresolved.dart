
class A {}

void f(A a) {
  ++a;
//^^
// [diag.undefinedOperator] The operator '+' isn't defined for the type 'A'.
}


void f((String,) a) {
  a + 0;
//  ^
// [diag.undefinedOperator] The operator '+' isn't defined for the type '(String,)'.
}

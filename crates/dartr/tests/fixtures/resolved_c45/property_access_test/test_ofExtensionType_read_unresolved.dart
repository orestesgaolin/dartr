
extension type A(int it) {}

void f(A a) {
  (a).foo;
//    ^^^
// [diag.undefinedGetter] The getter 'foo' isn't defined for the type 'A'.
}

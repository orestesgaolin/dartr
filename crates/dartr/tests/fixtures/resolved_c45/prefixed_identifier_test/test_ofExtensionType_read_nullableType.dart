
extension type A(int it) {
  int get foo => 0;
}

void f(A? a) {
  a.foo;
//  ^^^
// [diag.uncheckedPropertyAccessOfNullableValue] The property 'foo' can't be unconditionally accessed because the receiver can be 'null'.
}

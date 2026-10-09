
void f(({int bar}) r) {
  r.foo = 0;
//  ^^^
// [diag.undefinedSetter] The setter 'foo' isn't defined for the type '({int bar})'.
}

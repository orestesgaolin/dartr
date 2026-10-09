class A(int foo) {
  late var bar = foo;
//               ^^^
// [diag.undefinedIdentifier] Undefined name 'foo'.
}

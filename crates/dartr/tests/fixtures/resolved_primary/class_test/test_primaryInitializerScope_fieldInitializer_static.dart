class A(int foo) {
  static var bar = foo;
//                 ^^^
// [diag.undefinedIdentifier] Undefined name 'foo'.
}

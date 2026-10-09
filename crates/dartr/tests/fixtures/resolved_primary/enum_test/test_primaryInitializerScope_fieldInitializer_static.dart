enum A(int foo) {
  v(0);
  static var bar = foo;
//                 ^^^
// [diag.undefinedIdentifier] Undefined name 'foo'.
}

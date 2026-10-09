
f(Never a, int b) {
  a + b;
//^
// [diag.receiverOfTypeNever] The receiver is of type 'Never', and will never complete with a value.
//  ^^^
// [diag.deadCode] Dead code.
}

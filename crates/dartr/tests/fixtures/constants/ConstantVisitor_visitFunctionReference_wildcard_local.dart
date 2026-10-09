
test() {
  void _() {}
//^^^^^^^^^^^
// [diag.deadCode] Dead code.
  const c = _;
//          ^
// [diag.undefinedIdentifier] Undefined name '_'.
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
  print(c);
}

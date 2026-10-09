
test() {
  const _ = true;
  const c = _;
//      ^
// [diag.unusedLocalVariable] The value of the local variable 'c' isn't used.
//          ^
// [diag.undefinedIdentifier] Undefined name '_'.
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
}

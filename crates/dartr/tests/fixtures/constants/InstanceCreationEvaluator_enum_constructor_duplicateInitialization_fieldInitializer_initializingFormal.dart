
enum E {
  v(2);
  final int x = 1;
  const E(this.x);
//             ^
// [diag.finalInitializedInDeclarationAndConstructor] 'x' is final and was given a value when it was declared, so it can't be set to a new value.
}

const a = E.v;

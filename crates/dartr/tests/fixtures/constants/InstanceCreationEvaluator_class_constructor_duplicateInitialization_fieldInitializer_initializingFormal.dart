
class A {
  final int x = 1;
  const A(this.x);
//             ^
// [diag.finalInitializedInDeclarationAndConstructor] 'x' is final and was given a value when it was declared, so it can't be set to a new value.
}

const a = A(2);

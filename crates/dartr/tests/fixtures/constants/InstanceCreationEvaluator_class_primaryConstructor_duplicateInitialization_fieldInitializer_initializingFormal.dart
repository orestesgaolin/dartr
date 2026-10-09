
class const A(this.x) {
//                 ^
// [diag.fieldInitializedInDeclarationAndParameterOfPrimaryConstructor] Fields can't be initialized in both the primary constructor parameter list and at their declaration.
  final int x = 1;
}

const a = A(2);

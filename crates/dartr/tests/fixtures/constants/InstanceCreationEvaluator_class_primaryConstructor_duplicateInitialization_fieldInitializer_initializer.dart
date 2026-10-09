
class const A() {
  final int x = 1;
  this : x = 2;
//       ^
// [diag.fieldInitializedInDeclarationAndInitializerOfPrimaryConstructor] Fields can't be initialized in both the primary constructor and at their declaration.
}

const a = A();

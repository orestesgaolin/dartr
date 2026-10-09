
enum E {
  v;
  final int x = 1;
  const E() : x = 2;
//            ^
// [diag.fieldInitializedInInitializerAndDeclaration] Fields can't be initialized in the constructor if they are final and were already initialized at their declaration.
}

const a = E.v;


enum E {
  v;
  final int x;
  const E() : x = 1, x = 2;
//                   ^
// [diag.fieldInitializedByMultipleInitializers] The field 'x' can't be initialized twice in the same constructor.
}

const a = E.v;

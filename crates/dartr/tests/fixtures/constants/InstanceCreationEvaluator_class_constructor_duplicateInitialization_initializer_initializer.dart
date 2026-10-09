
class A {
  final int x;
  const A() : x = 1, x = 2;
//                   ^
// [diag.fieldInitializedByMultipleInitializers] The field 'x' can't be initialized twice in the same constructor.
}

const a = A();

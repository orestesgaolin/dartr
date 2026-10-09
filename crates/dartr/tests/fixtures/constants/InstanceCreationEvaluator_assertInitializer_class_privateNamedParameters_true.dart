
class A {
  final int _x;
//          ^^
// [diag.unusedField] The value of the field '_x' isn't used.
  const A({required this._x}) : assert(_x > 0);
}
const a = A(x: 1);

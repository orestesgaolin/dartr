
class A {
  final int _x;
//          ^^
// [diag.unusedField] The value of the field '_x' isn't used.
  final int _y;
//          ^^
// [diag.unusedField] The value of the field '_y' isn't used.
  const A({required this._x, required this._y}) : assert(_x < _y);
}
const a = A(x: 1, y: 2);

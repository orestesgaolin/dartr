
class C {
  final int _x;
  final int _y;
  const C({required this._x, required this._y});
  int get xy => _x + _y; // Avoid unused field warning.
}
const c = C(x: 123, y: 456);

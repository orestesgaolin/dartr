
class A {
  final int _;
  const A(this._);
  int x() => _; // Avoid unused field warning.
}
const a = const A(1);

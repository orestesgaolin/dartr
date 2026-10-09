
class A {
  final int _;
  const A(this._);
  int x() => _; // Avoid unused field warning.
}
class B extends A {
  const B(super._);
}
const a = const B(1);

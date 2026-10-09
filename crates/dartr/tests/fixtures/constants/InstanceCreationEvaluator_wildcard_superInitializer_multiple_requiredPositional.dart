
class A {
  final int _;
  final int y;
  const A(this._, this.y);
  int x() => _; // Avoid unused field warning.
}
class B extends A {
  const B(super._, super._);
}
const a = const B(1, 2);

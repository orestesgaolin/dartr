
class A {
  final int _;
//          ^
// [diag.unusedField] The value of the field '_' isn't used.
  final int y;
  const A([this._ = 1, this.y = 2]);
}
class B extends A {
  const B([super._ = 3, super._ = 4]);
}
const a = const B(10);

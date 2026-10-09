
class A {
  final int x;
  const A([this.x = 1]);
}
class const B([super.x]) extends A;
const b = B(2);

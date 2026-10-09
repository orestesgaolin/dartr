
class A {
  final int x;
  const A(this.x);
}
void fn() => 2;
const x = {A(0): 1, fn: 2};

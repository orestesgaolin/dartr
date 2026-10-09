
class const A(int x) {
  final int y = x + 1;
  const A.named(int z) : this(z * 2);
}
const a = A.named(10);


enum E<T> {
  v1<int>(10),
  v2(20),
  v3('abc');
  final T f;
  const E(this.f);
}

const x1 = E.v1;
const x2 = E.v2;
const x3 = E.v3;

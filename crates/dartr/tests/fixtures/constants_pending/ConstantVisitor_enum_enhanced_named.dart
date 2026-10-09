
enum E<T> {
  v1<double>.named(10),
  v2.named(20);
  final T f;
  const E.named(this.f);
}

const x1 = E.v1;
const x2 = E.v2;


extension type const E(int it) {}

class C<T> {
  final T f;
  const C(this.f);
}

const x = C(E(42));

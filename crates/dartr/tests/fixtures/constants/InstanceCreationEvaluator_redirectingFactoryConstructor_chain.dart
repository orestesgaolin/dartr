
class A {
  const factory A.foo(int a) = B<int>.bar;
}

class B<T> implements A {
  final T f;
  const B(this.f);
  const factory B.bar(T f) = C<T>;
}

class C<U> implements B<U> {
  final U f;
  const C(this.f);
}

const x = A.foo(0);

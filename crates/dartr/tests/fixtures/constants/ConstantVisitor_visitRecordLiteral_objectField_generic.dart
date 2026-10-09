
class A<T> {
  final (T, T) record;
  const A(T a) : record = (a, a);
}

const a = A(42);

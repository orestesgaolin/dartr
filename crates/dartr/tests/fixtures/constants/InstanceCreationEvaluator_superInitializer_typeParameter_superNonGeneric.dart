
class A {
  final Object f;
  const A(Object t): f = t;
}
class B<T> extends A {
  const B(): super(T);
}
const a = const B<int>();

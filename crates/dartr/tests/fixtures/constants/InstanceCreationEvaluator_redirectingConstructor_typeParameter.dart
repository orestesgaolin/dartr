
class A<T> {
  final Object f;
  const A(): this.named(T);
  const A.named(Object t): f = t;
}
const a = const A<int>();


void g<U>(U a) {}
class A<T> {
  final void Function(T) f;
  const A(): f = g;
}
const a = const A<int>();

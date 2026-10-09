
enum E { a, b }
class A {
  const A(E e) : assert(e != E.a);
}
const c = const A(E.b);

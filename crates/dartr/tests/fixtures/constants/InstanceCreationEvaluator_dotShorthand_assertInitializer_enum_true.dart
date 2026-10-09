
enum E { a, b }
class A {
  const A(E e) : assert(e != .a);
}
const A a = .new(.b);

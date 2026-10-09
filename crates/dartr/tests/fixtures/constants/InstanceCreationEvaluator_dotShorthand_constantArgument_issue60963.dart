
class A {
  const A();
}
extension type const B(A a) {}

const B b = .new(A());

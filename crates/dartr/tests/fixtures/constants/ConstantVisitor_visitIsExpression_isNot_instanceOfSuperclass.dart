
const a = const A();
const b = a is! B;
class A {
  const A();
}
class B extends A {
  const B();
}

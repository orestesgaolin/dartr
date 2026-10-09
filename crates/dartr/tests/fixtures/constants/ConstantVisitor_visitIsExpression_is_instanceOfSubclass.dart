
const a = const B();
const b = a is A;
//        ^^^^^^
// [diag.unnecessaryTypeCheckTrue] Unnecessary type check; the result is always 'true'.
class A {
  const A();
}
class B extends A {
  const B();
}

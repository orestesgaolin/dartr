
const a = const B();
const b = a is! A;
//        ^^^^^^^
// [diag.unnecessaryTypeCheckFalse] Unnecessary type check; the result is always 'false'.
class A {
  const A();
}
class B extends A {
  const B();
}

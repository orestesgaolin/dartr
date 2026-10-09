
const a = const A();
const b = a is! A;
//        ^^^^^^^
// [diag.unnecessaryTypeCheckFalse] Unnecessary type check; the result is always 'false'.
class A {
  const A();
}

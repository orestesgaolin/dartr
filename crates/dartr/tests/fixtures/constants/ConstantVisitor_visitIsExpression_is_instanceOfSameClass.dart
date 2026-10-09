
const a = const A();
const b = a is A;
//        ^^^^^^
// [diag.unnecessaryTypeCheckTrue] Unnecessary type check; the result is always 'true'.
class A {
  const A();
}

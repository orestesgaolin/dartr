
const a = const A();
const b = a as A;
//        ^^^^^^
// [diag.unnecessaryCast] Unnecessary cast.
class A {
  const A();
}

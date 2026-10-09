
const a = const A();
const b = a as B;
//        ^^^^^^
// [diag.constEvalThrowsException] Evaluation of this constant expression throws an exception.
class A {
  const A();
}
class B {
  const B();
}

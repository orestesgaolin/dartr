
class A {
  const A(double x): assert((x + 3) / 2 == 1.5);
//                   ^^^^^^^^^^^^^^^^^^^^^^^^^^
// [context 1] The exception is 'The assertion in this constant expression failed.' and occurs here.
}
const a = const A(1);
//        ^^^^^^^^^^
// [diag.constEvalThrowsException][context 1] Evaluation of this constant expression throws an exception.

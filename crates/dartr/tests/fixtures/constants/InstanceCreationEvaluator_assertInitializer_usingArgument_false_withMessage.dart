
class A {
  const A(int x): assert(x > 0, '$x must be greater than 0');
//                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [context 1] The exception is 'An assertion failed with message '0 must be greater than 0'.' and occurs here.
}
const a = const A(0);
//        ^^^^^^^^^^
// [diag.constEvalThrowsException][context 1] Evaluation of this constant expression throws an exception.

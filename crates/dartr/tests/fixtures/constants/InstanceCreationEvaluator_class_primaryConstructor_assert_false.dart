
class const A(int x) {
  this : assert(x > 0);
//       ^^^^^^^^^^^^^
// [context 1] The exception is 'The assertion in this constant expression failed.' and occurs here.
}
const a = A(0);
//        ^^^^
// [diag.constEvalThrowsException][context 1] Evaluation of this constant expression throws an exception.


enum E { a, b }
class A {
  const A(E e) : assert(e != E.a);
//               ^^^^^^^^^^^^^^^^
// [context 1] The exception is 'The assertion in this constant expression failed.' and occurs here.
}
const c = const A(E.a);
//        ^^^^^^^^^^^^
// [diag.constEvalThrowsException][context 1] Evaluation of this constant expression throws an exception.

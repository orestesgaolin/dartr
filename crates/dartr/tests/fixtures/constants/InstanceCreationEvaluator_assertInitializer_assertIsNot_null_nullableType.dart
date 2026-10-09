
class A<T> {
  const A() : assert(null is! T);
//            ^^^^^^^^^^^^^^^^^^
// [context 1] The exception is 'The assertion in this constant expression failed.' and occurs here.
}

const a = const A<int?>();
//        ^^^^^^^^^^^^^^^
// [diag.constEvalThrowsException][context 1] Evaluation of this constant expression throws an exception.

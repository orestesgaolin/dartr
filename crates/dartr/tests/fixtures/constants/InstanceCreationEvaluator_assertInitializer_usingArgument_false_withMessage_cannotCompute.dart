
class A {
  const A(int x): assert(x > 0, '${throw ''}');
//                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [context 1] The exception is 'The assertion in this constant expression failed.' and occurs here.
//                                 ^^^^^^^^
// [diag.invalidConstant] Invalid constant value.
// [diag.constConstructorThrowsException] Const constructors can't throw exceptions.
//                                          ^^^
// [diag.deadCode] Dead code.
}
const a = const A(0);
//        ^^^^^^^^^^
// [diag.constEvalThrowsException][context 1] Evaluation of this constant expression throws an exception.

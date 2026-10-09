
class A {
  final int _x;
//          ^^
// [diag.unusedField] The value of the field '_x' isn't used.
  const A({required this._x}) : assert(_x > 0);
//                              ^^^^^^^^^^^^^^
// [context 1] The exception is 'The assertion in this constant expression failed.' and occurs here.
}
const a = A(x: 0);
//        ^^^^^^^
// [diag.constEvalThrowsException][context 1] Evaluation of this constant expression throws an exception.


class C {
  final double d;
  const C(this.d);
}
class D extends C {
  const D(d) : super(d);
//      ^
// [context 1] The evaluated constructor 'C' is called by 'D' and 'D' is defined here.
//                   ^
// [context 3] The exception is 'A value of type 'String' can't be assigned to a parameter of type 'double' in a const constructor.' and occurs here.
}
class E extends D {
  const E(e) : super(e);
//      ^
// [context 2] The evaluated constructor 'D' is called by 'E' and 'E' is defined here.
}
const f = const E('0.0');
//        ^^^^^^^^^^^^^^
// [diag.constEvalThrowsException][context 1][context 2][context 3] Evaluation of this constant expression throws an exception.

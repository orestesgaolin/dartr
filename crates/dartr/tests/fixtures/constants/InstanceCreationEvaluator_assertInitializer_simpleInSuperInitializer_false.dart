
class A {
  const A(): assert(1 is String);
//           ^^^^^^^^^^^^^^^^^^^
// [context 2] The exception is 'The assertion in this constant expression failed.' and occurs here.
}
class B extends A {
  const B() : super();
//      ^
// [context 1] The evaluated constructor 'A' is called by 'B' and 'B' is defined here.
}
const b = const B();
//        ^^^^^^^^^
// [diag.constEvalThrowsException][context 1][context 2] Evaluation of this constant expression throws an exception.

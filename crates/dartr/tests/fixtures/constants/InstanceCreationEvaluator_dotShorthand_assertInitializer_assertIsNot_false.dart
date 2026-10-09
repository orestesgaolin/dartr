
class A {
  const A() : assert(0 is! int);
//            ^^^^^^^^^^^^^^^^^
// [context 1] The exception is 'The assertion in this constant expression failed.' and occurs here.
//                   ^^^^^^^^^
// [diag.unnecessaryTypeCheckFalse] Unnecessary type check; the result is always 'false'.
}

const A a = .new();
//          ^^^^^^
// [diag.constEvalThrowsException][context 1] Evaluation of this constant expression throws an exception.

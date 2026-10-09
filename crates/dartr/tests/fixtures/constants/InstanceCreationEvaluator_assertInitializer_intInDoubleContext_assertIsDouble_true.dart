
class A {
  const A(double x): assert(x is double);
//                          ^^^^^^^^^^^
// [diag.unnecessaryTypeCheckTrue] Unnecessary type check; the result is always 'true'.
}
const a = const A(0);

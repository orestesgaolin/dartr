
class A {
  const A(): assert(1 is int);
//                  ^^^^^^^^
// [diag.unnecessaryTypeCheckTrue] Unnecessary type check; the result is always 'true'.
}
const a = const A();

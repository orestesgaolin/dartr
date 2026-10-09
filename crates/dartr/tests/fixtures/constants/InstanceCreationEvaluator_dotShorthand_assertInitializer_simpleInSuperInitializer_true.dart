
class A {
  const A(): assert(1 is int);
//                  ^^^^^^^^
// [diag.unnecessaryTypeCheckTrue] Unnecessary type check; the result is always 'true'.
}
class B extends A {
  const B() : super();
}
const B b = .new();

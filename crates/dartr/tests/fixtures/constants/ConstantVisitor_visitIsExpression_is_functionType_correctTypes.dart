
void foo(int a) {}
const c = foo is void Function(int);
//        ^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.unnecessaryTypeCheckTrue] Unnecessary type check; the result is always 'true'.

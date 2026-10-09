
const a = null;
const b = a is dynamic;
//        ^^^^^^^^^^^^
// [diag.unnecessaryTypeCheckTrue] Unnecessary type check; the result is always 'true'.
class A {}

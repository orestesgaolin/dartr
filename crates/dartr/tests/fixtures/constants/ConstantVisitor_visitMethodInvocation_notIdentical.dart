
int f() {
  return 3;
}
const a = f();
//        ^^^
// [diag.constEvalMethodInvocation] Methods can't be invoked in constant expressions.

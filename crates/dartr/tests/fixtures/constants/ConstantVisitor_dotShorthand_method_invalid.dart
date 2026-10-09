
class A {
  static A method() => A();
}
const A a = .method();
//          ^^^^^^^^^
// [diag.constEvalMethodInvocation] Methods can't be invoked in constant expressions.

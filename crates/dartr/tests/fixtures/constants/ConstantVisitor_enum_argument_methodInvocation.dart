
enum E {
  enumValue(["text"].map((x) => x));
//          ^^^^^^^^^^^^^^^^^^^^^^
// [diag.constEvalMethodInvocation] Methods can't be invoked in constant expressions.

  const E(this.strings);
  final Iterable<String> strings;
}

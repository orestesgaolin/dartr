// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_const_nonConst_constructor).

class C {
  final int x;
  C.named(this.x);
}

void main() {
  C c = const .named(1);
//      ^^^^^
// [diag.constWithNonConst] The constructor being called isn't a const constructor.
  print(c);
}

// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_equality_pattern).

class C {
  final int x;
  const C.named(this.x);
}

void main() {
  C c = C.named(1);
  if (c case == const .named(2)) print('ok');
}

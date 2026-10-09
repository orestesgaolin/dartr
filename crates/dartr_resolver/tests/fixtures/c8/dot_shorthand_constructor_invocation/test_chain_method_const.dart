// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_chain_method_const).

class C {
  final int x;
  const C(this.x);
  C method() => C(1);
}

void main() {
  C c = const .new(1).method();
  print(c);
}

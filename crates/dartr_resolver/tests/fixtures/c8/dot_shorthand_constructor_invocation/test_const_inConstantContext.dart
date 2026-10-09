// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_const_inConstantContext).

class C {
  final int x;
  const C.named(this.x);
}

void main() {
  const C c = .named(1);
  print(c);
}

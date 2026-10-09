// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_new).

class C {
  int x;
  C(this.x);
}

void main() {
  C c = .new(1);
  print(c);
}

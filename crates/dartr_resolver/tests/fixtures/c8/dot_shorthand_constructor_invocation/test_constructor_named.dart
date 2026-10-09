// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_constructor_named).

class C {
  int x;
  C.named(this.x);
}

void main() {
  C c = .named(1);
  print(c);
}

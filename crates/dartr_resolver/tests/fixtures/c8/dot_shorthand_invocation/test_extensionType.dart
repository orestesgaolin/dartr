// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_extensionType).

extension type C(int integer) {
  static C one() => C(1);
}

void main() {
  C c = .one();
  print(c);
}

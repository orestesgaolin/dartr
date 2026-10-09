// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_equality_inferTypeParameters).

void main() {
  bool x = <int>[] == .filled(2, '2');
  print(x);
}

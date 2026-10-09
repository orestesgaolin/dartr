// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_requiredParameters_missing).

class C {
  int x;
  C({required this.x});
}

void main() {
  C c = .new();
//       ^^^
// [diag.missingRequiredArgument] The named parameter 'x' is required, but there's no corresponding argument.
  print(c);
}

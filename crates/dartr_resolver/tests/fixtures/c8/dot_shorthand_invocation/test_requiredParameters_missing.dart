// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_requiredParameters_missing).

class C {
  static C member({required int x}) => C(x);
  int x;
  C(this.x);
}

void main() {
  C c = .member();
//       ^^^^^^
// [diag.missingRequiredArgument] The named parameter 'x' is required, but there's no corresponding argument.
  print(c);
}

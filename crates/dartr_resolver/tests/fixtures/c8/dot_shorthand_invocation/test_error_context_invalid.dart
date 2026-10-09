// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_error_context_invalid).

class C { }

void main() {
  C Function() c = .member();
//                  ^^^^^^
// [diag.dotShorthandUndefinedInvocation] The static method or constructor 'member' isn't defined for the context type 'C Function()'.
  print(c);
}

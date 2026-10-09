// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_error_unresolved_new).

class C {
  C.named();
}

void main() {
  C c = .new();
//       ^^^
// [diag.dotShorthandUndefinedInvocation] The static method or constructor 'new' isn't defined for the context type 'C'.
  print(c);
}

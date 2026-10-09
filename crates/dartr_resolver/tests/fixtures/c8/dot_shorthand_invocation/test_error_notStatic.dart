// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_error_notStatic).

class C {
  C foo() => C();
}

void main() {
  final C c = .foo();
//             ^^^
// [diag.dotShorthandUndefinedInvocation] The static method or constructor 'foo' isn't defined for the context type 'C'.
  print(c);
}

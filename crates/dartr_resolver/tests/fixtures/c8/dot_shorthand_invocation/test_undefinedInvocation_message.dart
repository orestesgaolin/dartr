// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_undefinedInvocation_message).

int f() => .foo();
//          ^^^
// [diag.dotShorthandUndefinedInvocation] The static method or constructor 'foo' isn't defined for the context type 'int'.

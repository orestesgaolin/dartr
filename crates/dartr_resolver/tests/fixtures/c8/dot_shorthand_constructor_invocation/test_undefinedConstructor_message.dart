// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_undefinedConstructor_message).

int f() => const .foo();
//                ^^^
// [diag.constWithUndefinedConstructor] The class 'int' doesn't have a constant constructor 'foo'.

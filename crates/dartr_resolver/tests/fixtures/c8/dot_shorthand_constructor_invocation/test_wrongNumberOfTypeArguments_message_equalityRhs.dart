// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_wrongNumberOfTypeArguments_message_equalityRhs).

class C {}

bool f(C c) => c == .new<int>();
//                      ^^^^^
// [diag.wrongNumberOfTypeArgumentsDotShorthandConstructor] The dot shorthand resolves to the constructor 'C.new', and type parameters can't be applied to dot shorthand constructor invocations.

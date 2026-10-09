// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_typeParameters_missingContext).

void main() {
  var c = const .new<int>();
//        ^^^^^^^^^^^^^^^^^
// [diag.dotShorthandMissingContext] A dot shorthand can't be used where there is no context type.
  print(c);
}

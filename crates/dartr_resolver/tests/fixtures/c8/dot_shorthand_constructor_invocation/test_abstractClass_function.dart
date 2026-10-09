// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_abstractClass_function).

Function getFunction() {
  return .new();
//       ^^^^^^
// [diag.instantiateAbstractClass] Abstract classes can't be instantiated.
}

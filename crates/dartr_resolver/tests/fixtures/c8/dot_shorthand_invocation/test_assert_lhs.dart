// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_invocation_test.dart (DotShorthandInvocationResolutionTest.test_assert_lhs).

class C {
  final int x;
  const C.named(this.x);
}

class CAssert {
  const CAssert.regular(C ctor)
    : assert(const .named(1) == ctor);
//           ^^^^^^^^^^^^^^^
// [diag.dotShorthandMissingContext] A dot shorthand can't be used where there is no context type.
}

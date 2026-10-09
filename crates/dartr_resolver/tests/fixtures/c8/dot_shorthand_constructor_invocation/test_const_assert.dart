// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_const_assert).

class C {
  final int x;
  const C.named(this.x);
}

class CAssert {
  const CAssert.regular(C ctor)
    : assert(ctor == const .named(1));
}

// Ported from pkg/analyzer/test/src/dart/resolution/redirecting_constructor_invocation_test.dart (RedirectingConstructorInvocationResolutionTest.test_unnamed).

class C {
  C(int a);
  C.other() : this(0);
}

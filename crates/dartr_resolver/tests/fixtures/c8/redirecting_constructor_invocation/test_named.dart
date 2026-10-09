// Ported from pkg/analyzer/test/src/dart/resolution/redirecting_constructor_invocation_test.dart (RedirectingConstructorInvocationResolutionTest.test_named).

class C {
  C.named(int a);
  C.other() : this.named(0);
}

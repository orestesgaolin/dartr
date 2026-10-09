// Ported from pkg/analyzer/test/src/dart/resolution/redirecting_constructor_invocation_test.dart (RedirectingConstructorInvocationResolutionTest.test_named_unresolved_hasFormalParameter).

class C {
  C(int a);
  C.other(int named) : this.named(0);
}

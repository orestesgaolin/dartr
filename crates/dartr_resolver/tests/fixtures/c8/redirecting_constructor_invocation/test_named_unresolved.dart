// Ported from pkg/analyzer/test/src/dart/resolution/redirecting_constructor_invocation_test.dart (RedirectingConstructorInvocationResolutionTest.test_named_unresolved).

class C {
  C.other() : this.named(0);
//            ^^^^^^^^^^^^^
// [diag.redirectGenerativeToMissingConstructor] The constructor 'C.named' couldn't be found in 'C'.
}

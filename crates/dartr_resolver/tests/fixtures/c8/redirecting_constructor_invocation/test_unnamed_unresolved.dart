// Ported from pkg/analyzer/test/src/dart/resolution/redirecting_constructor_invocation_test.dart (RedirectingConstructorInvocationResolutionTest.test_unnamed_unresolved).

class C {
  C.named();
  C.other() : this(0);
//            ^^^^^^^
// [diag.redirectGenerativeToMissingConstructor] The constructor 'C' couldn't be found in 'C'.
}

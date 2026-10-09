// Ported from pkg/analyzer/test/src/dart/resolution/super_constructor_invocation_test.dart (SuperConstructorInvocationResolutionTest.test_named_unresolved_hasFormalParameter).

class A {
  A(int a);
}

class B extends A {
  B(int named) : super.named(0);
//               ^^^^^^^^^^^^^^
// [diag.undefinedConstructorInInitializer] The class 'A' doesn't have a constructor named 'named'.
}

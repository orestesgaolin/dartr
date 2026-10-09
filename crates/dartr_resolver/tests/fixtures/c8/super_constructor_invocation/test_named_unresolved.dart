// Ported from pkg/analyzer/test/src/dart/resolution/super_constructor_invocation_test.dart (SuperConstructorInvocationResolutionTest.test_named_unresolved).

class A {
  A(int a);
}

class B extends A {
  B() : super.named(0);
//      ^^^^^^^^^^^^^^
// [diag.undefinedConstructorInInitializer] The class 'A' doesn't have a constructor named 'named'.
}

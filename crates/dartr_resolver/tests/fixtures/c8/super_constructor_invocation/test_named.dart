// Ported from pkg/analyzer/test/src/dart/resolution/super_constructor_invocation_test.dart (SuperConstructorInvocationResolutionTest.test_named).

class A {
  A.named(int a);
}

class B extends A {
  B() : super.named(0);
}

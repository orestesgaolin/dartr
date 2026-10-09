// Ported from pkg/analyzer/test/src/dart/resolution/super_constructor_invocation_test.dart (SuperConstructorInvocationResolutionTest.test_unnamed).

class A {
  A(int a);
}

class B extends A {
  B() : super(0);
}

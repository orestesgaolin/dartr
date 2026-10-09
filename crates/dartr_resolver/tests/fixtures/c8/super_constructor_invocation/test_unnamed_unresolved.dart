// Ported from pkg/analyzer/test/src/dart/resolution/super_constructor_invocation_test.dart (SuperConstructorInvocationResolutionTest.test_unnamed_unresolved).

class A {
  A.named(int a);
}

class B extends A {
  B() : super(0);
//      ^^^^^^^^
// [diag.undefinedConstructorInInitializerDefault] The class 'A' doesn't have an unnamed constructor.
}

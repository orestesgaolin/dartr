// Ported from pkg/analyzer/test/src/dart/resolution/super_constructor_invocation_test.dart (SuperConstructorInvocationResolutionTest.test_nonConst_fromConst).

class A {
  final a;
  A(this.a);
}

class B extends A {
  const B() : super(5);
//            ^^^^^^^^
// [diag.constConstructorWithNonConstSuper] A constant constructor can't call a non-constant super constructor of 'A'.
}

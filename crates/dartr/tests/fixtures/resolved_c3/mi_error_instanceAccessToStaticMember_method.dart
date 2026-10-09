// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_error_instanceAccessToStaticMember_method).

class A {
  static void foo(int _) {}
}

void f(A a) {
  a.foo(0);
//  ^^^
// [diag.instanceAccessToStaticMember] The static method 'foo' can't be accessed through an instance.
}

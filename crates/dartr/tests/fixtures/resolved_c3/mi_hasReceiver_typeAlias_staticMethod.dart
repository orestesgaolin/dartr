// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_typeAlias_staticMethod).

class A {
  static void foo(int _) {}
}

typedef B = A;

void f() {
  B.foo(0);
}

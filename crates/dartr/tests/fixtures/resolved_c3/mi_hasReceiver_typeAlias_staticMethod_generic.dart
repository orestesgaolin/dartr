// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_typeAlias_staticMethod_generic).

class A<T> {
  static void foo(int _) {}
}

typedef B<T> = A<T>;

void f() {
  B.foo(0);
}

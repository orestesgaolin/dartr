// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_interfaceType_enum_fromMixin).

mixin M on Enum {
  void foo() {}
}

enum E with M {
  v;
}

void f(E e) {
  e.foo();
}

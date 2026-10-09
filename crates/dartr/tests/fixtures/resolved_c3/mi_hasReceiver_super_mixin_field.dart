// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_super_mixin_field).

class A {
  int foo() => 0;
}

mixin M on A {
  late final v = super.foo();
}

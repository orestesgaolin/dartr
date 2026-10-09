// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_hasReceiver_super_class_field).

class A {
  int foo() => 0;
}

class B extends A {
  late final v = super.foo();
}

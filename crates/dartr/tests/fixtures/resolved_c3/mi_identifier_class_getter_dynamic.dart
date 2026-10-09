// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_identifier_class_getter_dynamic).

class A {
  dynamic get foo => null;

  void f() {
    foo(0);
  }
}

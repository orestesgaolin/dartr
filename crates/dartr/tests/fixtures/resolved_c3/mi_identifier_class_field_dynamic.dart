// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_identifier_class_field_dynamic).

class A {
  dynamic foo;

  void f() {
    foo(0);
  }
}

// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_parameterMember_source).

void foo<T>({int? a}) {}

void f() {
  foo(a: 0);
}

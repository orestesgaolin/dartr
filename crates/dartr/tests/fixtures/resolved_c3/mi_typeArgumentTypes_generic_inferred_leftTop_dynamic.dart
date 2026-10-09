// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_typeArgumentTypes_generic_inferred_leftTop_dynamic).

void foo<T extends Object>(T? value) {}

void f(dynamic o) {
  foo(o);
}

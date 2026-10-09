// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_typeArgumentTypes_generic_inferred_leftTop_void).

void foo<T extends Object>(List<T?> value) {}

void f(List<void> o) {
  foo(o);
}

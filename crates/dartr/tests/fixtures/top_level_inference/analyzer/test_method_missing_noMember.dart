// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_method_missing_noMember).
class A {
  int foo(String a) => null;
}
class B extends A {
  m(a) {}
}

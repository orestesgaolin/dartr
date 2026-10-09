// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_method_missing_hasMethod_noParameter_named).
class A {
  void m(int a) {}
}
class B extends A {
  m(a, {b}) {}
}

// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_method_error_hasMethod_noParameter_required).
class A {
  void m(int a) {}
}
class B extends A {
  void m(a, b) {}
}

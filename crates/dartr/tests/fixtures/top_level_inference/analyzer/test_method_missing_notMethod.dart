// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_method_missing_notMethod).
class A {
  int m = 42;
}
class B extends A {
  m(a) {}
}

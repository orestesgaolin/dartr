// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_method_OK_single_extends_direct_generic).
class A<K, V> {
  V m(K a, double b) {}
}
class B extends A<int, String> {
  m(a, b) {}
}

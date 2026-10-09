// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_method_OK_single_extends_indirect_generic).
class A<K, V> {
  V m(K a) {}
}
class B<T> extends A<int, T> {}
class C extends B<String> {
  m(a) {}
}

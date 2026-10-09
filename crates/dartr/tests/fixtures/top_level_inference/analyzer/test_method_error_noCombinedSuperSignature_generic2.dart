// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_method_error_noCombinedSuperSignature_generic2).
class A<K, V> {
  V m(K a) {}
}
class B<T> {
  T m(int a) {}
}
class C extends A<int, String> implements B<double> {
  m(a) {}
}

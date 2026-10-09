// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_method_OK_single_implements_indirect_generic).
abstract class A<K, V> {
  V m(K a);
}
abstract class B<T1, T2> extends A<T2, T1> {}
class C implements B<int, String> {
  m(a) {}
}

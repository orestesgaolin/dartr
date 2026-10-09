// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_method_OK_two_extendsImplements_generic).
class A<K, V> {
  V m(K a) {}
}
class B<T> {
  T m(int a) {}
}
class C extends A<int, String> implements B<String> {
  m(a) {}
}

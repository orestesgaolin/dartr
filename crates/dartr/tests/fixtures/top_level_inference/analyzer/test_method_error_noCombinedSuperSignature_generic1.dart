// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_method_error_noCombinedSuperSignature_generic1).
class A<T> {
  void m(T a) {}
}
class B<E> {
  void m(E a) {}
}
class C extends A<int> implements B<double> {
  m(a) {}
}

// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_method_error_noCombinedSuperSignature1).
class A {
  void m(int a) {}
}
class B {
  void m(String a) {}
}
class C extends A implements B {
  m(a) {}
}

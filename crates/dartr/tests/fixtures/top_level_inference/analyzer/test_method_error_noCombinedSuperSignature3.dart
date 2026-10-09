// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_method_error_noCombinedSuperSignature3).
class A {
  int m() {}
}
class B {
  String m() {}
}
class C extends A implements B {
  m() {}
}

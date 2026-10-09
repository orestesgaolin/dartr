// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_method_OK_single_extends_direct_notGeneric).
class A {
  String m(int a) {}
}
class B extends A {
  m(a) {}
}

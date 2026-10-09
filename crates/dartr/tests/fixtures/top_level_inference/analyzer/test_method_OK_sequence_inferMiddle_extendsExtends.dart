// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_method_OK_sequence_inferMiddle_extendsExtends).
class A {
  String m(int a) {}
}
class B extends A {
  m(a) {}
}
class C extends B {
  m(a) {}
}

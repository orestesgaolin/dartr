// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_extractProperty_inStaticField).
class A {
  int f;
}
class B {
  static var t = new A().f;
}

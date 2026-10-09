// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_identifier_error_cycle_classField).
class A {
  static var a = B.b;
}
class B {
  static var b = A.a;
}
var c = A.a;

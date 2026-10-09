// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_identifier_error_cycle_mix).
class A {
  static var a = b;
}
var b = A.a;
var c = b;

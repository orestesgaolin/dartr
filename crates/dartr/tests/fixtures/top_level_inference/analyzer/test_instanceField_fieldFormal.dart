// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_instanceField_fieldFormal).
class A {
  var f = 0;
  A([this.f = 'hello']);
}

// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_prefix_incDec_custom).
class A {
  B operator+(int v) => null;
}
class B {}
var a = new A();
var vInc = ++a;
var vDec = --a;

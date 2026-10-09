// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_assign_prefixed).
class A {
  int f;
}
var a = new A();
var t1 = (a.f = 1);
var t2 = (a.f += 2);

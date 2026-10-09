// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_assign_prefixed_viaInterface).
class I {
  int f;
}
abstract class C implements I {}
C c;
var t1 = (c.f = 1);
var t2 = (c.f += 2);

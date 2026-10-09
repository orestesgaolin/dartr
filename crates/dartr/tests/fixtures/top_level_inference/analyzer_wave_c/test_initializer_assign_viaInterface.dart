// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_assign_viaInterface).
class I {
  int f;
}
abstract class C implements I {}
C getC() => null;
var t1 = (getC().f = 1);
var t2 = (getC().f += 2);

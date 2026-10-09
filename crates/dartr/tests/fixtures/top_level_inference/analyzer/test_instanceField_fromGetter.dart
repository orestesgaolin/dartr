// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_instanceField_fromGetter).
abstract class A {
  int get x;
  int get y;
  int get z;
}
class B implements A {
  var x;
  get y => null;
  set z(_) {}
}

// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_instanceField_fromSetter).
abstract class A {
  void set x(int _);
  void set y(int _);
  void set z(int _);
}
class B implements A {
  var x;
  get y => null;
  set z(_) {}
}

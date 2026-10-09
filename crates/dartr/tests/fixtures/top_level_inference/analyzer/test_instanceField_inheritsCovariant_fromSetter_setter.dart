// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_instanceField_inheritsCovariant_fromSetter_setter).
abstract class A {
  num get x;
  void set x(covariant num _);
}
class B implements A {
  set x(int _) {}
}

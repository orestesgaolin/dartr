// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_instanceField_inheritsCovariant_fromSetter_field).
abstract class A {
  num get x;
  void set x(covariant num _);
}
class B implements A {
  int x;
}

// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_instanceField_error_noSetterParameter).
abstract class A {
  int x;
}
class B implements A {
  set x() {}
}

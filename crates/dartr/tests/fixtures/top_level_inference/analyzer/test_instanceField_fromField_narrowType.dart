// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_instanceField_fromField_narrowType).
abstract class A {
  num x;
}
class B implements A {
  var x = 1;
}

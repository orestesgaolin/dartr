// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_instanceField_fromField_implicitDynamic).
abstract class A {
  var x;
}
class B implements A {
  var x = 1;
}

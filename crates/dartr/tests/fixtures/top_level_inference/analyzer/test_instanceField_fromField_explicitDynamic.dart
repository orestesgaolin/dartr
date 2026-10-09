// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_instanceField_fromField_explicitDynamic).
abstract class A {
  dynamic x;
}
class B implements A {
  var x = 1;
}

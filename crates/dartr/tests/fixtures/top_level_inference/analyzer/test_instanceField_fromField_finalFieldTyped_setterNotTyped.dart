// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_instanceField_fromField_finalFieldTyped_setterNotTyped).
abstract class A {
  int? foo;
}
class B implements A {
  final int foo;
  set foo(_) {}
}

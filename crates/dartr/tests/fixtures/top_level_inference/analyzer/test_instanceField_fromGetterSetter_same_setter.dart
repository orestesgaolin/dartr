// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_instanceField_fromGetterSetter_same_setter).
abstract class A {
  int get x;
}
abstract class B {
  void set x(int _);
}
class C implements A, B {
  set x(_);
}

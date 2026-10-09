// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_classField_static_instanceCreation).
class A<T> {}
class B {
  static var t1 = 1;
  static var t2 = new A();
}

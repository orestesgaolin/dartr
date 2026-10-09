// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_classField_instance_instanceCreation).
class A<T> {}
class B {
  var t1 = new A<int>();
  var t2 = new A();
}

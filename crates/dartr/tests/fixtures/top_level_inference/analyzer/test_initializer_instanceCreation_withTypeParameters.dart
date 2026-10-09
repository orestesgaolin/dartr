// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_instanceCreation_withTypeParameters).
class A<T> {}
var t1 = new A<int>();
var t2 = new A();

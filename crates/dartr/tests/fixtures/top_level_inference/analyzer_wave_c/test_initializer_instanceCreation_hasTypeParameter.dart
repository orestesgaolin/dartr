// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_instanceCreation_hasTypeParameter).
class A<T> {}
var a = new A<int>();
var b = new A();

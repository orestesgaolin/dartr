// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_methodInvocation_noTypeParameters).
class A {
  String m(int p) => null;
}
var instanceOfA = new A();
var v1 = instanceOfA.m();
var v2 = new A().m();

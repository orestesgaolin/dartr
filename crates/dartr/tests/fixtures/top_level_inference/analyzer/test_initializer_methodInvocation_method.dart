// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_methodInvocation_method).
class A {
  int m1() => 0;
  T m2<T>() => throw 0;
}
var a = new A();
var t1 = a.m1();
var t2 = a.m2();
var t3 = a.m2<int>();

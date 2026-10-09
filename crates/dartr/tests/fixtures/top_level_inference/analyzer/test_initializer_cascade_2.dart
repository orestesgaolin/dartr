// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_cascade_2).
class A {
  int a;
  void m() {}
}
var vSetField = new A()..a = 1;
var vInvokeMethod = new A()..m();
var vBoth = new A()..a = 1..m();

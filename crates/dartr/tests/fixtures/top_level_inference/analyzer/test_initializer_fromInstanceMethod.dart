// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_fromInstanceMethod).
class A {
  int foo() => 0;
}
class B extends A {
  foo() => 1;
}
var x = A().foo();
var y = B().foo();

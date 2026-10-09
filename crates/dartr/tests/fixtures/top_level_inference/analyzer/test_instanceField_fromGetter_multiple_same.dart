// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_instanceField_fromGetter_multiple_same).
abstract class A {
  int get x;
}
abstract class B {
  int get x;
}
class C implements A, B {
  get x => null;
}

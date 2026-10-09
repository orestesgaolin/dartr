// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_instanceField_fromGetter_multiple_different_generic).
abstract class A<T> {
  T get x;
}
abstract class B<T> {
  T get x;
}
class C implements A<int>, B<String> {
  get x => null;
}

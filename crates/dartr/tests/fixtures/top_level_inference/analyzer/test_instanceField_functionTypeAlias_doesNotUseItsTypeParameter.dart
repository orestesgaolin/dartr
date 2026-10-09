// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_instanceField_functionTypeAlias_doesNotUseItsTypeParameter).
typedef F<T>();

class A<T> {
  F<T> get x => null;
  List<F<T>> get y => null;
}

class B extends A<int> {
  get x => null;
  get y => null;
}

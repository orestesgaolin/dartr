// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_methodInvocation_hasTypeParameters).
class A {
  List<T> m<T>() => null;
}
var vWithTypeArgument = new A().m<int>();
var vWithoutTypeArgument = new A().m();

// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_functionInvocation_hasTypeParameters).
T f<T>() => null;
var vHasTypeArgument = f<int>();
var vNoTypeArgument = f();

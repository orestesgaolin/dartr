// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_functionInvocation_noTypeParameters).
String f(int p) => null;
var vOkArgumentType = f(1);
var vWrongArgumentType = f(2.0);

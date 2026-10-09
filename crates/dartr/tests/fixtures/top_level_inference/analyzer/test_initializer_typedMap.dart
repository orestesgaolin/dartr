// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_typedMap).
var a = 1;
var t = <int, int>{(a = 1) : (a = 2)};

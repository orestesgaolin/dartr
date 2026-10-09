// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_conditional).
var a = 1;
var b = true;
var t = b
    ? (a = 1)
    : (a = 2);

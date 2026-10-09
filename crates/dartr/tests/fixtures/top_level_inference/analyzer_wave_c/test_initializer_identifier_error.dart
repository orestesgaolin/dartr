// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_identifier_error).
var a = 0;
var b = (a = 1);
var c = b;

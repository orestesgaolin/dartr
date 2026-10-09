// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_equality).
var a = 1;
var t1 = ((a = 1) == 0) == ((a = 2) == 0);
var t2 = ((a = 1) == 0) != ((a = 2) == 0);

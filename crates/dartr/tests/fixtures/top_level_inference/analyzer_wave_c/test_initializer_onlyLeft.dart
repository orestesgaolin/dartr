// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_onlyLeft).
var a = 1;
var vEq = a == ((a = 2) == 0);
var vNotEq = a != ((a = 2) == 0);

// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_assign_indexed).
var a = [0];
var t1 = (a[0] = 2);
var t2 = (a[0] += 2);

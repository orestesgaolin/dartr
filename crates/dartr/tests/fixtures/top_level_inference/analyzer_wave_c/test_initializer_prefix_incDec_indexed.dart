// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_prefix_incDec_indexed).
var vInt = [1];
var vDouble = [2.0];
var vIncInt = ++vInt[0];
var vDecInt = --vInt[0];
var vIncDouble = ++vDouble[0];
var vDecInt = --vDouble[0];

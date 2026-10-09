// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_dependencyCycle).
var a = b;
//  ^
// [diag.topLevelCycle] The type of 'a' can't be inferred because it depends on itself through the cycle: a, b.
var b = a;
//  ^
// [diag.topLevelCycle] The type of 'b' can't be inferred because it depends on itself through the cycle: a, b.

// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_identifier_error_cycle_topLevel).
final a = b;
final b = c;
final c = a;
final d = a;

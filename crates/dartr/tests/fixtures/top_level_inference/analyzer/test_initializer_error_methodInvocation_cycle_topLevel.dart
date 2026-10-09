// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_error_methodInvocation_cycle_topLevel).
var a = b.foo();
var b = a.foo();

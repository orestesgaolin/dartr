// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_instanceGetterOfObject).
dynamic f() => null;
var s = f().toString();
var h = f().hashCode;

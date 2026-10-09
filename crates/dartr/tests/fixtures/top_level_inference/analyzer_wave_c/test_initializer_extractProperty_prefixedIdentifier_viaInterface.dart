// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_extractProperty_prefixedIdentifier_viaInterface).
class I {
  bool b;
}
abstract class C implements I {}
C c;
var x = c.b;

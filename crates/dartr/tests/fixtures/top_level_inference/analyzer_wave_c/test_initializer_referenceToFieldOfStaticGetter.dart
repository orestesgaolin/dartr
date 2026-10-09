// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_referenceToFieldOfStaticGetter).
class C {
  static D get d => null;
}
class D {
  int i;
}
var x = C.d.i;

// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_referenceToFieldOfStaticField).
class C {
  static D d;
}
class D {
  int i;
}
final x = C.d.i;

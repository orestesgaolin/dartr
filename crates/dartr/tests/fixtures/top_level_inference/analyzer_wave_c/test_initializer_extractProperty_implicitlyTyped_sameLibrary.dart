// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_initializer_extractProperty_implicitlyTyped_sameLibrary).
class C {
  var f = 0;
}
var x = new C().f;

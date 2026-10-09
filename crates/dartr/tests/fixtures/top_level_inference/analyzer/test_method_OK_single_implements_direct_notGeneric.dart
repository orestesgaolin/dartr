// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_method_OK_single_implements_direct_notGeneric).
abstract class A {
  String m(int a);
}
class B implements A {
  m(a) {}
}

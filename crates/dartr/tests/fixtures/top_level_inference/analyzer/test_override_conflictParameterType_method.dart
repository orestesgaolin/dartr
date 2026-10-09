// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_override_conflictParameterType_method).
abstract class A {
  void mmm(int a);
}
abstract class B {
  void mmm(String a);
}
class C implements A, B {
  void mmm(a) {}
//     ^^^
// [diag.noCombinedSuperSignature] Can't infer missing types in 'C' from overridden methods: A.mmm (void Function(int)), B.mmm (void Function(String)).
}

// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_method_error_noCombinedSuperSignature2).
abstract class A {
  int foo(int x);
}

abstract class B {
  double foo(int x);
}

abstract class C implements A, B {
  Never foo(x);
}

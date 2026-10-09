// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_method_OK_two_extendsImplements_notGeneric).
class A {
  String m(int a) {}
}
class B {
  String m(int a) {}
}
class C extends A implements B {
  m(a) {}
}

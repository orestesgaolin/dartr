// Ported from pkg/analyzer/test/src/summary/top_level_inference_test.dart
// (test_instanceField_fromGetter_hasGetterWithType_hasSetterNoType).
abstract class A {
  num get foo;
}
class B implements A {
  int get foo => 0;
  set foo(value) {}
}

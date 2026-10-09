// Dart source: pkg/analyzer/test/src/dart/resolution/metadata_test.dart test_value_genericClass_inference_unnamedConstructor
 class A<T> {
  final T f;
  const A(this.f);
}

@A(42)
void f() {}

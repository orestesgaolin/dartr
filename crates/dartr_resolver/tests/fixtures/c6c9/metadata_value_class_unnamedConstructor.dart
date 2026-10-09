// Dart source: pkg/analyzer/test/src/dart/resolution/metadata_test.dart test_value_class_unnamedConstructor
class A {
  final int f;
  const A(this.f);
}

@A(42)
void f() {}

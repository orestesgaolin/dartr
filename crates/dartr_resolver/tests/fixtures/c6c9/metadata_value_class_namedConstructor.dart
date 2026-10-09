// Dart source: pkg/analyzer/test/src/dart/resolution/metadata_test.dart test_value_class_namedConstructor
 class A {
  final int f;
  const A.named(this.f);
}

@A.named(42)
void f() {}

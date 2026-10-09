// Dart source: pkg/analyzer/test/src/dart/resolution/metadata_test.dart test_value_class_staticConstField
class A {
  static const int foo = 42;
}

@A.foo
void f() {}

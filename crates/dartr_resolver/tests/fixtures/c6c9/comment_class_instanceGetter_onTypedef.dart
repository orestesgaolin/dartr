// Dart source: pkg/analyzer/test/src/dart/resolution/comment_test.dart test_class_instanceGetter_onTypedef
class A {
  int get foo => 0;
}
typedef B = A;

/// [B.foo]
void f() {}

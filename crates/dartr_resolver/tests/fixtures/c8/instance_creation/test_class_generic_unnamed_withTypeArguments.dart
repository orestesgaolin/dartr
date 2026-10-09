// Ported from pkg/analyzer/test/src/dart/resolution/instance_creation_test.dart (InstanceCreationTestCases.test_class_generic_unnamed_withTypeArguments).

class A<T> {}

void f() {
  A<int>();
}

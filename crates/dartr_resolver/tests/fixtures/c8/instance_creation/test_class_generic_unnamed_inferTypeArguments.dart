// Ported from pkg/analyzer/test/src/dart/resolution/instance_creation_test.dart (InstanceCreationTestCases.test_class_generic_unnamed_inferTypeArguments).

class A<T> {
  A(T t);
}

void f() {
  A(0);
}

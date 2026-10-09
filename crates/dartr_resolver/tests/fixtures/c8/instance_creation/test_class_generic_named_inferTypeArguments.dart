// Ported from pkg/analyzer/test/src/dart/resolution/instance_creation_test.dart (InstanceCreationTestCases.test_class_generic_named_inferTypeArguments).

class A<T> {
  A.named(T t);
}

void f() {
  A.named(0);
}

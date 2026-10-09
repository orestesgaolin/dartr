// Ported from pkg/analyzer/test/src/dart/resolution/instance_creation_test.dart (InstanceCreationTestCases.test_class_generic_named_withTypeArguments).

class A<T> {
  A.named();
}

void f() {
  A<int>.named();
}

// Ported from pkg/analyzer/test/src/dart/resolution/instance_creation_test.dart (InstanceCreationTestCases.test_class_notGeneric_named).

class A {
  A.named(int a);
}

void f() {
  A.named(0);
}

// Ported from pkg/analyzer/test/src/dart/resolution/instance_creation_test.dart (InstanceCreationTestCases.test_unnamed_declaredNew).

class A {
  A.new(int a);
}

void f() {
  A(0);
}


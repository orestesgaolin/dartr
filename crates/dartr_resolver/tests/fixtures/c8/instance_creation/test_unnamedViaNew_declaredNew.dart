// Ported from pkg/analyzer/test/src/dart/resolution/instance_creation_test.dart (InstanceCreationTestCases.test_unnamedViaNew_declaredNew).

class A {
  A.new(int a);
}

void f() {
  A.new(0);
}


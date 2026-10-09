// Ported from pkg/analyzer/test/src/dart/resolution/instance_creation_test.dart (InstanceCreationTestCases.test_arguments_named).

class A {
  A(int a, {required bool b, required double c});
}

void f() {
  A(0, b: true, c: 1.2);
}

// Ported from pkg/analyzer/test/src/dart/resolution/instance_creation_test.dart (InstanceCreationTestCases.test_extensionType_generic_primary_unnamed).

extension type A<T>(T it) {}

void f() {
  A(0);
}

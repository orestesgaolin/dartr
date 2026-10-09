// Ported from pkg/analyzer/test/src/dart/resolution/instance_creation_test.dart (InstanceCreationTestCases.test_extensionType_notGeneric_primary_unnamed).

extension type A(int it) {}

void f() {
  A(0);
}

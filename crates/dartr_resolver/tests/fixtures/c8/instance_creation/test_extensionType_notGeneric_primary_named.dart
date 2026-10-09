// Ported from pkg/analyzer/test/src/dart/resolution/instance_creation_test.dart (InstanceCreationTestCases.test_extensionType_notGeneric_primary_named).

extension type A.named(int it) {}

void f() {
  A.named(0);
}

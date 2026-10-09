// Ported from pkg/analyzer/test/src/dart/resolution/instance_creation_test.dart (InstanceCreationTestCases.test_extensionType_notGeneric_secondary_named).

extension type A(int it) {
  A.named(this.it);
}

void f() {
  A.named(0);
}

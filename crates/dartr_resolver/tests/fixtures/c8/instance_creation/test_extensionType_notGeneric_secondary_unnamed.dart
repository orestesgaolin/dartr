// Ported from pkg/analyzer/test/src/dart/resolution/instance_creation_test.dart (InstanceCreationTestCases.test_extensionType_notGeneric_secondary_unnamed).

extension type A.named(int it) {
  A(this.it);
}

void f() {
  A(0);
}

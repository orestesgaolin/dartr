// Ported from pkg/analyzer/test/src/dart/resolution/instance_creation_test.dart (InstanceCreationTestCases.test_extensionType_generic_secondary_unnamed).

extension type A<T>.named(T it) {
  A(this.it);
}

void f() {
  A(0);
}

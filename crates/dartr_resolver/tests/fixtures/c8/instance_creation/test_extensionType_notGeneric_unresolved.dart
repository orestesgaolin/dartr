// Ported from pkg/analyzer/test/src/dart/resolution/instance_creation_test.dart (InstanceCreationTestCases.test_extensionType_notGeneric_unresolved).

extension type A(int it) {}

void f() {
  new A.named(0);
//      ^^^^^
// [diag.newWithUndefinedConstructor] The class 'A' doesn't have a constructor named 'named'.
}

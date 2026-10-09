// Ported from pkg/analyzer/test/src/dart/resolution/instance_creation_test.dart (InstanceCreationTestCases.test_unresolved_identifier).

void f() {
  new Unresolved.named(0);
//    ^^^^^^^^^^^^^^^^
// [diag.undefinedIdentifier] Undefined name 'Unresolved'.
}


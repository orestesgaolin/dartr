// Ported from pkg/analyzer/test/src/dart/resolution/instance_creation_test.dart (InstanceCreationTestCases.test_unresolved_identifier_identifier).

void f() {
  new unresolved.Foo.bar(0);
//    ^^^^^^^^^^^^^^
// [diag.undefinedIdentifier] Undefined name 'unresolved'.
}


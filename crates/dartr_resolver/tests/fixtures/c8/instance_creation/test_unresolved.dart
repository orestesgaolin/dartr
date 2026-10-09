// Ported from pkg/analyzer/test/src/dart/resolution/instance_creation_test.dart (InstanceCreationTestCases.test_unresolved).

void f() {
  new Unresolved(0);
//    ^^^^^^^^^^
// [diag.newWithNonType] The name 'Unresolved' isn't a class.
}


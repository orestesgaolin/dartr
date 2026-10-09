// Ported from pkg/analyzer/test/src/dart/resolution/instance_creation_test.dart (InstanceCreationTestCases.test_class_notGeneric_unresolved).

class A {}

void f() {
  new A.unresolved(0);
//      ^^^^^^^^^^
// [diag.newWithUndefinedConstructor] The class 'A' doesn't have a constructor named 'unresolved'.
}


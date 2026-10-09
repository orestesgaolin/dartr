// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_syntheticName).

class A {
  A() : B(1 + 2, [0]);
//      ^
// [diag.missingAssignmentInInitializer] Expected an assignment after the field name.
//      ^^^^^^^^^^^^^
// [diag.initializerForNonExistentField] 'B' isn't a field in the enclosing class.
}

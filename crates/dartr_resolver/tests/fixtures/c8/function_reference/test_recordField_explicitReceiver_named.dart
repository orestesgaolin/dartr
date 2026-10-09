// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_recordField_explicitReceiver_named).

void f(({T Function<T>(T) f1, String f2}) r) {
  int Function(int) v = r.f1;
//                  ^
// [diag.unusedLocalVariable] The value of the local variable 'v' isn't used.
}

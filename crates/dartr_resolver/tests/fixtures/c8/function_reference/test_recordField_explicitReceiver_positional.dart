// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_recordField_explicitReceiver_positional).

void f((T Function<T>(T), String) r) {
  int Function(int) v = r.$1;
//                  ^
// [diag.unusedLocalVariable] The value of the local variable 'v' isn't used.
}

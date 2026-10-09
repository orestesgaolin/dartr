// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceMethod_explicitReceiver_receiverIsNotIdentifier_call).

extension on List<Object?> {
  void foo<T>(T a) {}
}

var a = [].foo.call<int>;

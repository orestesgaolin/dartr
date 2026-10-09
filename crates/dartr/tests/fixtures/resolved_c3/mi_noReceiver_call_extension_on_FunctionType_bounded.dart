// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_noReceiver_call_extension_on_FunctionType_bounded).

extension E<T extends int Function()> on T {
  void f() {
    call();
  }
}

// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_noReceiver_call_extension_on_FunctionType).

extension E on int Function() {
  void f() {
    call();
  }
}

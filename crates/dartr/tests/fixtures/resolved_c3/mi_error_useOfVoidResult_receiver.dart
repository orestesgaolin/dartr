// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_error_useOfVoidResult_receiver).

main() {
  void foo;
  foo.toString();
//^^^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
}

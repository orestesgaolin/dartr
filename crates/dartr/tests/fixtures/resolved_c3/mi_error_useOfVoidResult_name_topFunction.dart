// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_error_useOfVoidResult_name_topFunction).

void foo() {}

main() {
  foo()();
//^^^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
}

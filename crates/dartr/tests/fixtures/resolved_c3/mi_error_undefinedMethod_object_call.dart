// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_error_undefinedMethod_object_call).

main(Object o) {
  o.call();
//  ^^^^
// [diag.undefinedMethod] The method 'call' isn't defined for the type 'Object'.
}

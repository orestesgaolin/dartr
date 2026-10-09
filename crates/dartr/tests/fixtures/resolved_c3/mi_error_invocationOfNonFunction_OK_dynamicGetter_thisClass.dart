// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_error_invocationOfNonFunction_OK_dynamicGetter_thisClass).

class C {
  var foo;

  main() {
    foo();
  }
}

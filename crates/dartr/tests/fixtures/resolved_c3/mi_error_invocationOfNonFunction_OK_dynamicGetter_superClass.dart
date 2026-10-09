// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_error_invocationOfNonFunction_OK_dynamicGetter_superClass).

class A {
  var foo;
}

class B extends A {
  main() {
    foo();
  }
}

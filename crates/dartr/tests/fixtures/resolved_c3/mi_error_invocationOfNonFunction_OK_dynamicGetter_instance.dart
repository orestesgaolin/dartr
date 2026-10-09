// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_error_invocationOfNonFunction_OK_dynamicGetter_instance).

class C {
  var foo;
}

void f(C c) {
  c.foo();
}

// Ported from pkg/analyzer/test/src/dart/resolution/function_expression_invocation_test.dart (test_call_infer_fromArguments).

class A {
  void call<T>(T t) {}
}

void f(A a) {
  a(0);
}

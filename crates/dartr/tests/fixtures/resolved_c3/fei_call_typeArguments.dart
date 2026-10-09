// Ported from pkg/analyzer/test/src/dart/resolution/function_expression_invocation_test.dart (test_call_typeArguments).

class A {
  T call<T>() {
    throw 42;
  }
}

void f(A a) {
  a<int>();
}

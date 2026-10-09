// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_expression_interfaceType_explicitCall).

class C {
  double call(int p) => 0.0;
}

void f(C c) {
  c.call(0);
}

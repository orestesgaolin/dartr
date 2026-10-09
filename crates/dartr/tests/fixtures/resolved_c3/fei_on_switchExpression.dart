// Ported from pkg/analyzer/test/src/dart/resolution/function_expression_invocation_test.dart (test_on_switchExpression).

void f(Object? x) {
  (switch (x) {
    _ => foo,
  }());
}

void foo() {}

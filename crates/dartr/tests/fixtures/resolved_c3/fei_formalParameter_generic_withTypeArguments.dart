// Ported from pkg/analyzer/test/src/dart/resolution/function_expression_invocation_test.dart (test_formalParameter_generic_withTypeArguments).

typedef F<S> = S Function<T>(T x);

void f(F<int> a) {
  a<String>('hello');
}

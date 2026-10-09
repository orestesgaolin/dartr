// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceMethod_explicitReceiver_variable_promoted).

void f(num n) {
  num x = n;
  if (x is int) {
    x.expectStaticType<Exactly<int>>;
  }
}

extension StaticType<T> on T {
  void expectStaticType<X extends Exactly<T>>() {}
}

typedef Exactly<T> = T Function(T);

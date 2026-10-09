// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_error_invocationOfNonFunction_OK_functionTypeTypeParameter).

typedef MyFunction = double Function(int _);

class C<T extends MyFunction> {
  T foo;
  C(this.foo);

  main() {
    foo(0);
  }
}

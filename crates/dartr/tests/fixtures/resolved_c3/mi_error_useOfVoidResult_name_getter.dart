// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_error_useOfVoidResult_name_getter).

class C<T>{
  T foo;
  C(this.foo);
}

void f(C<void> c) {
  c.foo();
//^^^^^
// [diag.useOfVoidResult] This expression has a type of 'void' so its value can't be used.
}

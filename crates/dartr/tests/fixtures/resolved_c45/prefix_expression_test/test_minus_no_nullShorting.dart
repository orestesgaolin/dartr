
class A {
  int get foo => 0;
}

void f(A? a) {
  -a?.foo;
//^
// [diag.uncheckedMethodInvocationOfNullableValue] The method 'unary-' can't be unconditionally invoked because the receiver can be 'null'.
}

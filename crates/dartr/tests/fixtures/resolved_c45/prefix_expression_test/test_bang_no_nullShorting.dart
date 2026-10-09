
class A {
  bool get foo => true;
}

void f(A? a) {
  !a?.foo;
// ^^^^^^
// [diag.uncheckedUseOfNullableValueAsCondition] A nullable expression can't be used as a condition.
}

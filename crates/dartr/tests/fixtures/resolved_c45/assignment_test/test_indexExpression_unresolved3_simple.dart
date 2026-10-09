
class A {
  operator[]=(int index, num _) {}
}

void f(A a, int c) {
  a[b] = c;
//  ^
// [diag.undefinedIdentifier] Undefined name 'b'.
}

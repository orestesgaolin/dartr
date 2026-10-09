
class A {
  int? get foo => 0;
}

main() {
  A a = A()..foo?.isEven;
  a;
}

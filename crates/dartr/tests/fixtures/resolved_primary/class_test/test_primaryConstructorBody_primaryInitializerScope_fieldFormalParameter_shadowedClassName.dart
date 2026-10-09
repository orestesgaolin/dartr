class A {
  A(int _);
}

class B(this.A) {
  final int Function() A;
  this : assert(A() > 0);
}

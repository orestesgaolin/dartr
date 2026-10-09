
class A {
  static int get x => 0;
  static set x(int _) {}
}

typedef B = A;

void f() {
  B.x += 2;
}


extension E on int {
  bool operator[](int index) => false;
}

void f() {
  0[1];
}

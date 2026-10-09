
extension E on int {}

void f() {
  0[1];
}

augment extension E {
  bool operator[](int index) => false;
}

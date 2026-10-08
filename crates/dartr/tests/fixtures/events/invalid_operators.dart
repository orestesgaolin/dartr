class A {
  operator +(a) {}
  operator ===(a) {}
  operator !==(a) {}
  bool operator ~/(o) => true;
  operator [](i) => 1;
  operator []=(i, v) {}
  operator -() => this;
  operator >>>(i) => 1;
  operator >>(i) => 1;
}
var x = a and b or c xor d shl e shr f;

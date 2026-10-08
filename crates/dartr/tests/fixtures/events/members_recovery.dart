class A {
  operator ==(o) => true;
  void operator +(x) {}
  get x => 1;
  set (v) {}
  static static int y;
  final var z;
  const factory A.f() = B;
  A() : x = 1, super(), this.y = 2;
  external A.e() : super();
  int get get => 0;
  class B {}
  enum C {}
  typedef T = int;
  foo bar baz;
  )
}

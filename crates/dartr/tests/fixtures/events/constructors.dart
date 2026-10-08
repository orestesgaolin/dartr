class A {
  A();
  A.named() : this();
  const A.c(this.x) : assert(x > 0);
  factory A.f() => A();
  factory A.r() = B.named;
  A.init() : x = 1, y = 2 {}
  A.bad() : x;
  A.bad2() : ;
  A.super() : super.named();
  A.new();
  new A2();
}

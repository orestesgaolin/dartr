
A? get foo => A();

class A {
  A get bar => this;
  A? get baz => this;
  A get baq => this;
}

main() {
  foo?.bar?..baz?.baq;
}

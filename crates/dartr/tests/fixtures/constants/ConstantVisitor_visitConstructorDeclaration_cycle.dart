
class A {
  final A a;
  const A() : a = const A();
//      ^
// [diag.recursiveConstantConstructor] The constant constructor depends on itself.
}


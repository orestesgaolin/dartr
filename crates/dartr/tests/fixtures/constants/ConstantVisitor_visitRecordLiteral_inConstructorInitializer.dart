
class A {
  final bool b;
  const A(r) : b = r is (int, ) ? true : true;
}

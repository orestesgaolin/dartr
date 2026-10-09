
class A {
  int x = 0;
}
abstract class I<T> {
  T x = throw 0;
}
class B extends A implements I<int> {}
main() {
  new B().x = 1;
}

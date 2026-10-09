
abstract class A {
  T Function<T>(T) operator[](int i);
}
abstract class B {
  A get a;
}
int Function(int)? f(B? b) => b?.a[0];

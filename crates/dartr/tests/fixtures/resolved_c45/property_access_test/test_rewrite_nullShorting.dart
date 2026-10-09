
abstract class A {
  T Function<T>(T) get f;
}
abstract class B {
  A get a;
}
int Function(int)? f(B? b) => b?.a.f;

// Input of the `interface` differential test
// (crates/dartr_typesystem/tests/interface_dump_test.rs). Only `Object` and
// type arguments come from the SDK, so the mock SDK of the Rust test has the
// same members as the real SDK. Every member has an explicit type (no
// inference).

class A<T> {
  T foo(T a) => a;
  int get bar => 0;
  set bar(int value) {}
  void _private() {}
}

mixin M<U> on A<U> {
  U foo(U a) => a;
  String baz() => '';
}

class B extends A<int> with M<int> {}

abstract class I1 {
  Object? get x;
  void m(int a);
  void n(Object? a, dynamic b);
}

abstract class I2 {
  dynamic get x;
  void m(num a);
  void n(dynamic a, Object? b);
}

abstract class C implements I1, I2 {}

abstract class G {
  int get g;
}

abstract class H {
  int g();
}

abstract class D implements G, H {}

class E implements I1 {
  Object? get x => null;
  dynamic noSuchMethod(Invocation invocation) => null;
}

class P {
  void foo(covariant num a) {}
}

class Q {
  void foo(int a) {}
}

class R extends Q implements P {}

extension type X1(A<int> it) implements A<int> {
  void own() {}
}

abstract class S<K, V> {
  Map<K, V> get map;
  V lookUp(K key);
}

abstract class SS extends S<String, List<int>> {}

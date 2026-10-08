// Override inference of methods and their parameters
// (InstanceMemberInferrer._inferExecutable).

abstract class A {
  int m1(String a, [double b]);
  num m2({int x, required String y});
  T generic<T extends num>(T t, List<T> list);
  void covariantParams(covariant num a, {covariant num named});
  dynamic operator [](int index);
  void operator []=(int index, String value);
  Object? objectQ(Object? o);
  int arity(int a);
}

abstract class B implements A {
  m1(a, [b]);
  m2({x, required y});
  generic<S extends num>(s, list);
  covariantParams(int a, {int named});
  operator [](index);
  operator []=(index, value);
  objectQ(o);
  arity(a, [b]);
}

abstract class GenericMismatch implements A {
  generic(t, list);
}

abstract class Kinds {
  int get g;
  int f = 0;
}

abstract class MethodOverridesGetter implements Kinds {
  g();
  f();
}

abstract class C1 {
  int m(int a);
  Object? top(dynamic x);
}

abstract class C2 {
  String m(int a);
  dynamic top(Object? x);
}

abstract class Conflict implements C1, C2 {
  m(a);
  top(x);
}

abstract class NoSignature implements C1, C2 {
  int m(a);
}

abstract class Equality {
  bool operator ==(other);
}

abstract class Equality2 extends Equality {
  bool operator ==(other);
}

class ExplicitEq {
  bool operator ==(Object other) => true;
}

class ExplicitEq2 extends ExplicitEq {
  bool operator ==(other) => true;
}

class Hash {
  get hashCode => 0;
  toString() => '';
  noSuchMethod(i) => null;
}

abstract class Generic<T> {
  T m(T a, {List<T> b});
}

abstract class Specialized extends Generic<String> {
  m(a, {b});
}

abstract class Covariant1 {
  void m(covariant int a, [num b]);
}

abstract class Covariant2 implements Covariant1 {
  void m(int a, [num b]);
}

abstract class Covariant3 extends Covariant2 {
  void m(int a, [num b]);
}

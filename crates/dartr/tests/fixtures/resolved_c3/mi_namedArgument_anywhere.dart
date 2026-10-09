// Ported from pkg/analyzer/test/src/dart/resolution/method_invocation_test.dart (test_namedArgument_anywhere).

class A {}
class B {}
class C {}
class D {}

void foo(A a, B b, {C? c, D? d}) {}

T g1<T>() => throw 0;
T g2<T>() => throw 0;
T g3<T>() => throw 0;
T g4<T>() => throw 0;

void f() {
  foo(g1(), c: g3(), g2(), d: g4());
}

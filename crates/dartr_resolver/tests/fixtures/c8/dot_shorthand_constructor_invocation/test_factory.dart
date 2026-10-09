// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_factory).

class Foo<T> {
  factory Foo.a() = _Foo;

  Foo();
}

class _Foo<T> extends Foo<T> {
  _Foo();
}

Foo<T> bar<T>() => .a();

// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_factory_const).

class Foo<T> {
  const factory Foo.a() = _Foo;

  const Foo();
}

class _Foo<T> extends Foo<T> {
  const _Foo();
}

Foo<T> bar<T>() => const .a();

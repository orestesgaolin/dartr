// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_abstractClass_factory_const_typeArguments).

abstract class Foo<T> {
  const factory Foo.a() = _Foo;

  const Foo();
}

class _Foo<T> extends Foo<T> {
  const _Foo();
}

Foo<int> bar<T>() => const .a<int>();
//                           ^^^^^
// [diag.wrongNumberOfTypeArgumentsDotShorthandConstructor] The dot shorthand resolves to the constructor 'Foo.a', and type parameters can't be applied to dot shorthand constructor invocations.

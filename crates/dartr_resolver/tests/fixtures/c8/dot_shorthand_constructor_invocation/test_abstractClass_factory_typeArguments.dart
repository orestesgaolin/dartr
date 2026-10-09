// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_abstractClass_factory_typeArguments).

abstract class Foo<T> {
  factory Foo.a() = _Foo;

  Foo();
}

class _Foo<T> extends Foo<T> {
  _Foo();
}

Foo<T> bar<T>() => .a<T>();
//                   ^^^
// [diag.wrongNumberOfTypeArgumentsDotShorthandConstructor] The dot shorthand resolves to the constructor 'Foo.a', and type parameters can't be applied to dot shorthand constructor invocations.

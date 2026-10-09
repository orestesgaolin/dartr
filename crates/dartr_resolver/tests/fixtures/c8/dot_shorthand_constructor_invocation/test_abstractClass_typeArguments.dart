// Ported from pkg/analyzer/test/src/dart/resolution/dot_shorthand_constructor_invocation_test.dart (DotShorthandConstructorInvocationResolutionTest.test_abstractClass_typeArguments).

abstract class Foo<T> {
  Foo();
}

void main() {
  Foo _ = .new<int>();
//        ^^^^^^^^^^^
// [diag.instantiateAbstractClass] Abstract classes can't be instantiated.
}

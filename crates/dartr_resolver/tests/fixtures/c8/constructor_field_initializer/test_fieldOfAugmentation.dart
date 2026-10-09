// Ported from pkg/analyzer/test/src/dart/resolution/constructor_field_initializer_test.dart (ConstructorFieldInitializerResolutionTest.test_fieldOfAugmentation).

class A {
  int get foo;
}

augment class A {
  final int _foo;

  const A() : _foo = 0;

  augment int get foo => _foo;
}

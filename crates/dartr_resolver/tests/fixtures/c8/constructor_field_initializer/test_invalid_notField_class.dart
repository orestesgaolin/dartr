// Ported from pkg/analyzer/test/src/dart/resolution/constructor_field_initializer_test.dart (ConstructorFieldInitializerResolutionTest.test_invalid_notField_class).

class A {
  const A() : X = a;
//            ^^^^^
// [diag.initializerForNonExistentField] 'X' isn't a field in the enclosing class.
}
const a = 0;
class X {}

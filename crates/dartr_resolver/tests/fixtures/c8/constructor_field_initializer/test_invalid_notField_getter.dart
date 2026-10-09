// Ported from pkg/analyzer/test/src/dart/resolution/constructor_field_initializer_test.dart (ConstructorFieldInitializerResolutionTest.test_invalid_notField_getter).

class A {
  A() : x = a;
//      ^^^^^
// [diag.initializerForNonExistentField] 'x' isn't a field in the enclosing class.
  int get x => 0;
}
const a = 0;

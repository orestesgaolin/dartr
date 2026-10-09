// Ported from pkg/analyzer/test/src/dart/resolution/constructor_field_initializer_test.dart (ConstructorFieldInitializerResolutionTest.test_invalid_notField_topLevelVariable).

class A {
  A() : x = a;
//      ^^^^^
// [diag.initializerForNonExistentField] 'x' isn't a field in the enclosing class.
}
const a = 0;
var x = 0;

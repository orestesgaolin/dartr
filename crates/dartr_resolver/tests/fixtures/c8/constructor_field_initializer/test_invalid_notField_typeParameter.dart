// Ported from pkg/analyzer/test/src/dart/resolution/constructor_field_initializer_test.dart (ConstructorFieldInitializerResolutionTest.test_invalid_notField_typeParameter).

class A<T> {
  A() : T = a;
//      ^^^^^
// [diag.initializerForNonExistentField] 'T' isn't a field in the enclosing class.
}
const a = 0;

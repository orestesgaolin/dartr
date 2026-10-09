// Ported from pkg/analyzer/test/src/dart/resolution/constructor_reference_test.dart (ConstructorReferenceResolutionTest_TypeArgs.test_class_generic_new_typeArgs).

class A<T> {
  A.new();
}

void bar() {
  A<int>.new<int>;
//          ^^^^^
// [diag.wrongNumberOfTypeArgumentsConstructor] The constructor 'A.new' doesn't have type parameters.
}

// Ported from pkg/analyzer/test/src/dart/resolution/constructor_reference_test.dart (ConstructorReferenceResolutionTest_TypeArgs.test_class_generic_named_cascade).

class A<T> {
  A.foo();
}

void bar() {
  A<int>..foo;
// ^
// [diag.undefinedOperator] The operator '<' isn't defined for the type 'Type'.
//     ^
// [diag.equalityCannotBeEqualityOperand] A comparison expression can't be an operand of another comparison expression.
//      ^^
// [diag.missingIdentifier] Expected an identifier.
}

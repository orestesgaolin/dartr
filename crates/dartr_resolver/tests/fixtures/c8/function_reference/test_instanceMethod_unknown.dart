// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceMethod_unknown).

class A {
  bar() {
    foo<int>;
//  ^^^
// [diag.undefinedMethod] The method 'foo' isn't defined for the type 'A'.
  }
}

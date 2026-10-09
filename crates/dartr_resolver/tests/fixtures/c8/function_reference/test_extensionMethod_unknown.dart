// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_extensionMethod_unknown).

extension on double {
  bar() {
//^^^
// [diag.unusedElement] The declaration 'bar' isn't referenced.
    foo<int>;
//  ^^^
// [diag.undefinedMethod] The method 'foo' isn't defined for the type 'double'.
  }
}

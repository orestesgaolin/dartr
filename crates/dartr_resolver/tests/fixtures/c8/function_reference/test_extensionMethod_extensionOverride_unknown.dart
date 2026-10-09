// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_extensionMethod_extensionOverride_unknown).

class A {}

extension E on A {}

bar(A a) {
  E(a).foo<int>;
//     ^^^
// [diag.undefinedExtensionGetter] The getter 'foo' isn't defined for the extension 'E'.
}

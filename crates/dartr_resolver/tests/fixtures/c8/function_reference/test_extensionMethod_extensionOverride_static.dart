// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_extensionMethod_extensionOverride_static).

class A {}

extension E on A {
  static void foo<T>(T a) {}
}

bar(A a) {
  E(a).foo<int>;
//     ^^^
// [diag.extensionOverrideAccessToStaticMember] An extension override can't be used to access a static member from an extension.
}

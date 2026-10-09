// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_extensionMethod_extensionOverride_cascade).

class A {
  int foo = 0;
}

extension E on A {
  void foo<T>(T a) {}
}

bar(A a) {
  E(a)..foo<int>;
//^
// [diag.extensionOverrideWithCascade] Extension overrides have no value so they can't be used as the receiver of a cascade expression.
}

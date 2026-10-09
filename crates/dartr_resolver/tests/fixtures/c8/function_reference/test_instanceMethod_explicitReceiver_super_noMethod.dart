// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceMethod_explicitReceiver_super_noMethod).

class A {
  bar() {
    super.foo<int>;
//        ^^^
// [diag.undefinedSuperGetter] The getter 'foo' isn't defined in a superclass of 'A'.
  }
}

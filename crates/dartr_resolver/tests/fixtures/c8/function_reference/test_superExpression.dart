// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_superExpression).

class A {
  void call<T>() {}
}

class B extends A {
  void f() {
    super<int>;
//  ^^^^^
// [diag.missingAssignableSelector] Missing selector such as '.identifier' or '[0]'.
  }
}

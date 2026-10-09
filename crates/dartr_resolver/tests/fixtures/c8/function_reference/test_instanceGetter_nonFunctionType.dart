// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceGetter_nonFunctionType).

abstract class A {
  List<int> get f;
}

void foo(A a) {
  a.f<String>;
//  ^
// [diag.disallowedTypeInstantiationExpression] Only a generic type, generic function, generic instance method, or generic constructor can have type arguments.
}

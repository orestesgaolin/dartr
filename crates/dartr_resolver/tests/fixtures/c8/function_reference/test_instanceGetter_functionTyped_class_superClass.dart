// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceGetter_functionTyped_class_superClass).

abstract class A {
  late void Function<T>(T) foo;
}

abstract class B extends A {
  void f() {
    foo<int>;
  }
}

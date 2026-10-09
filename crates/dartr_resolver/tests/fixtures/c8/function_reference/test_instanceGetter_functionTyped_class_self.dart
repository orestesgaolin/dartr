// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceGetter_functionTyped_class_self).

abstract class A {
  late void Function<T>(T) foo;

  bar() {
    foo<int>;
  }
}


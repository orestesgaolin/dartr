// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_instanceGetter_explicitReceiver).

class A {
  late void Function<T>(T) foo;
}

bar(A a) {
  a.foo<int>;
}

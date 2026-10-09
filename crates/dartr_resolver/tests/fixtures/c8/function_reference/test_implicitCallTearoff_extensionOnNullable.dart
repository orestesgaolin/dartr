// Ported from pkg/analyzer/test/src/dart/resolution/function_reference_test.dart (FunctionReferenceResolutionTest.test_implicitCallTearoff_extensionOnNullable).

Object? v = null;
extension E on Object? {
  void call<R, S>(R r, S s) {}
}
void foo() {
  v<int, String>;
}


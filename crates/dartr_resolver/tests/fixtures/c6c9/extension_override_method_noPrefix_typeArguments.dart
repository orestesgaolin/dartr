// Dart source: pkg/analyzer/test/src/dart/resolution/extension_override_test.dart test_method_noPrefix_typeArguments
class A {}
extension E<T> on A {
  void m() {}
}
void f(A a) {
  E<int>(a).m();
}

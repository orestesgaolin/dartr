// Dart source: pkg/analyzer/test/src/dart/resolution/extension_override_test.dart test_getter_noPrefix_typeArguments
class A {}
extension E<T> on A {
  int get g => 0;
}
void f(A a) {
  E<int>(a).g;
}
